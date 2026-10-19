//! Android window-inset notifications. View access is handled by a UI-thread observer.
use android_activity::AndroidApp;
use jni::{
    JNIEnv, JavaVM, NativeMethod,
    objects::{GlobalRef, JClass, JObject},
    sys::{jint, jlong},
};
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{
        Arc, LazyLock, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
use winit::dpi::PhysicalInsets;

type Handler = Arc<dyn Fn(u64, PhysicalInsets<u32>) + Send + Sync>;
static HANDLERS: LazyLock<Mutex<HashMap<u64, Handler>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

extern "system" fn on_insets(
    _env: JNIEnv,
    _class: JClass,
    id: jlong,
    left: jint,
    top: jint,
    right: jint,
    bottom: jint,
) {
    // A callback must never unwind across the JNI boundary. Release the registry
    // lock before invoking the handler, including while a view is being dropped.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let handler = HANDLERS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&(id as u64))
            .cloned();
        if let Some(handler) = handler {
            handler(
                id as u64,
                PhysicalInsets {
                    left: left.max(0) as u32,
                    top: top.max(0) as u32,
                    right: right.max(0) as u32,
                    bottom: bottom.max(0) as u32,
                },
            );
        }
    }));
    if result.is_err() {
        eprintln!("Android window-inset callback panicked");
    }
}

fn observer_class(env: &mut JNIEnv, activity: &JObject) -> jni::errors::Result<GlobalRef> {
    static CLASS: OnceLock<GlobalRef> = OnceLock::new();
    if let Some(class) = CLASS.get() {
        return Ok(class.clone());
    }
    let class = super::android_java::load_class(env, activity, "dev.blitz.shell.InsetsObserver")?;
    env.register_native_methods(
        <&JClass>::from(class.as_obj()),
        &[NativeMethod {
            name: "onInsets".into(),
            sig: "(JIIII)V".into(),
            fn_ptr: on_insets as *mut c_void,
        }],
    )?;
    let _ = CLASS.set(class.clone());
    Ok(class)
}

pub(crate) struct AndroidInsetsMonitor {
    id: u64,
    observer: GlobalRef,
    vm: JavaVM,
}
impl AndroidInsetsMonitor {
    pub(crate) fn new(
        app: &AndroidApp,
        handler: impl Fn(u64, PhysicalInsets<u32>) + Send + Sync + 'static,
    ) -> jni::errors::Result<Self> {
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }?;
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        HANDLERS.lock().unwrap().insert(id, Arc::new(handler));
        let result = (|| {
            let mut env = vm.attach_current_thread()?;
            env.with_local_frame(16, |env| -> jni::errors::Result<GlobalRef> {
                let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
                let class = observer_class(env, &activity)?;
                let class: &JClass = class.as_obj().into();
                let observer = env
                    .call_static_method(
                        class,
                        "attach",
                        "(Landroid/app/Activity;J)Ldev/blitz/shell/InsetsObserver;",
                        &[(&activity).into(), (id as jlong).into()],
                    )?
                    .l()?;
                env.new_global_ref(observer)
            })
        })();
        match result {
            Ok(observer) => Ok(Self { id, observer, vm }),
            Err(error) => {
                HANDLERS.lock().unwrap().remove(&id);
                Err(error)
            }
        }
    }
    pub(crate) fn id(&self) -> u64 {
        self.id
    }
}
impl Drop for AndroidInsetsMonitor {
    fn drop(&mut self) {
        HANDLERS.lock().unwrap().remove(&self.id);
        if let Ok(mut env) = self.vm.attach_current_thread() {
            let _ = env.call_method(self.observer.as_obj(), "close", "()V", &[]);
        }
    }
}
