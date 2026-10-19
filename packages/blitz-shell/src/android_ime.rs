//! A standard Android InputConnection forwards editor snapshots to the Rust event loop.
use super::android_ime_state::AndroidEditorState;
use android_activity::AndroidApp;
use jni::{
    JNIEnv, JavaVM, NativeMethod,
    objects::{GlobalRef, JClass, JObject, JString},
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

#[derive(Debug, Clone)]
pub enum AndroidImeEvent {
    State {
        text: String,
        selection: (i32, i32),
        composition: Option<(i32, i32)>,
    },
    Action(i32),
}

type Handler = Arc<dyn Fn(u64, u64, u64, AndroidImeEvent) + Send + Sync>;
static HANDLERS: LazyLock<Mutex<HashMap<u64, Handler>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn send(id: jlong, session: jlong, sequence: jlong, event: AndroidImeEvent) {
    let handler = HANDLERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&(id as u64))
        .cloned();
    if let Some(handler) = handler {
        handler(id as u64, session as u64, sequence as u64, event);
    }
}
extern "system" fn on_state(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    session: jlong,
    sequence: jlong,
    text: JString,
    start: jint,
    end: jint,
    compose_start: jint,
    compose_end: jint,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let text = match env.get_string(&text) {
            Ok(text) => String::from(text),
            Err(error) => {
                eprintln!("Android IME string conversion failed: {error}");
                return;
            }
        };
        send(
            id,
            session,
            sequence,
            AndroidImeEvent::State {
                text,
                selection: (start, end),
                composition: if (compose_start, compose_end) == (-1, -1) {
                    None
                } else {
                    Some((compose_start, compose_end))
                },
            },
        );
    }));
    if result.is_err() {
        eprintln!("Android IME state callback panicked");
    }
}
extern "system" fn on_action(
    _: JNIEnv,
    _: JClass,
    id: jlong,
    session: jlong,
    sequence: jlong,
    action: jint,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        send(id, session, sequence, AndroidImeEvent::Action(action))
    }));
    if result.is_err() {
        eprintln!("Android IME action callback panicked");
    }
}
fn host_class(env: &mut JNIEnv, activity: &JObject) -> jni::errors::Result<GlobalRef> {
    static CLASS: OnceLock<GlobalRef> = OnceLock::new();
    if let Some(class) = CLASS.get() {
        return Ok(class.clone());
    }
    let class = super::android_java::load_class(env, activity, "dev.blitz.shell.TextInputHost")?;
    env.register_native_methods(
        <&JClass>::from(class.as_obj()),
        &[
            NativeMethod {
                name: "onState".into(),
                sig: "(JJJLjava/lang/String;IIII)V".into(),
                fn_ptr: on_state as *mut c_void,
            },
            NativeMethod {
                name: "onAction".into(),
                sig: "(JJJI)V".into(),
                fn_ptr: on_action as *mut c_void,
            },
        ],
    )?;
    let _ = CLASS.set(class.clone());
    Ok(class)
}

#[derive(Clone, PartialEq, Eq)]
struct Snapshot {
    session: u64,
    acknowledged: u64,
    state: AndroidEditorState,
    input_type: i32,
    options: i32,
}

pub(crate) struct AndroidImeBridge {
    id: u64,
    host: GlobalRef,
    vm: JavaVM,
    previous: Option<Snapshot>,
}
impl AndroidImeBridge {
    pub fn new(
        app: &AndroidApp,
        handler: impl Fn(u64, u64, u64, AndroidImeEvent) + Send + Sync + 'static,
    ) -> jni::errors::Result<Self> {
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }?;
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        HANDLERS.lock().unwrap().insert(id, Arc::new(handler));
        let result = (|| {
            let mut env = vm.attach_current_thread()?;
            env.with_local_frame(16, |env| -> jni::errors::Result<GlobalRef> {
                let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
                let class = host_class(env, &activity)?;
                let host = env
                    .call_static_method(
                        <&JClass>::from(class.as_obj()),
                        "attach",
                        "(Landroid/app/Activity;J)Ldev/blitz/shell/TextInputHost;",
                        &[(&activity).into(), (id as jlong).into()],
                    )?
                    .l()?;
                env.new_global_ref(host)
            })
        })();
        match result {
            Ok(host) => Ok(Self {
                id,
                host,
                vm,
                previous: None,
            }),
            Err(error) => {
                HANDLERS.lock().unwrap().remove(&id);
                Err(error)
            }
        }
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn show(&self, session: u64) -> jni::errors::Result<()> {
        let mut env = self.vm.attach_current_thread()?;
        env.call_method(
            self.host.as_obj(),
            "show",
            "(J)V",
            &[(session as jlong).into()],
        )?;
        Ok(())
    }
    pub fn update(
        &mut self,
        session: u64,
        acknowledged: u64,
        state: AndroidEditorState,
        input_type: i32,
        options: i32,
    ) -> jni::errors::Result<()> {
        let snapshot = Snapshot {
            session,
            acknowledged,
            state,
            input_type,
            options,
        };
        if self.previous.as_ref() == Some(&snapshot) {
            return Ok(());
        }
        let mut env = self.vm.attach_current_thread()?;
        env.with_local_frame(8, |env| -> jni::errors::Result<()> {
            let text = env.new_string(&snapshot.state.text)?;
            let (cs, ce) = snapshot.state.composition.unwrap_or((-1, -1));
            env.call_method(
                self.host.as_obj(),
                "update",
                "(JJLjava/lang/String;IIIIII)V",
                &[
                    (session as jlong).into(),
                    (acknowledged as jlong).into(),
                    (&text).into(),
                    snapshot.state.selection.0.into(),
                    snapshot.state.selection.1.into(),
                    cs.into(),
                    ce.into(),
                    input_type.into(),
                    options.into(),
                ],
            )?;
            Ok(())
        })?;
        self.previous = Some(snapshot);
        Ok(())
    }
}
impl Drop for AndroidImeBridge {
    fn drop(&mut self) {
        HANDLERS.lock().unwrap().remove(&self.id);
        if let Ok(mut env) = self.vm.attach_current_thread() {
            let _ = env.call_method(self.host.as_obj(), "close", "()V", &[]);
        }
    }
}
