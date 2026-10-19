//! Load the shell's embedded Java helpers using the Activity's class loader.
use jni::{
    JNIEnv,
    objects::{GlobalRef, JObject},
};
use std::sync::OnceLock;

pub(crate) fn load_class(
    env: &mut JNIEnv,
    activity: &JObject,
    name: &str,
) -> jni::errors::Result<GlobalRef> {
    static LOADER: OnceLock<GlobalRef> = OnceLock::new();
    let loader = if let Some(loader) = LOADER.get() {
        loader.clone()
    } else {
        let bytes =
            env.byte_array_from_slice(include_bytes!(concat!(env!("OUT_DIR"), "/classes.dex")))?;
        let buffer = env
            .call_static_method(
                "java/nio/ByteBuffer",
                "wrap",
                "([B)Ljava/nio/ByteBuffer;",
                &[(&bytes).into()],
            )?
            .l()?;
        let parent = env
            .call_method(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
            .l()?;
        let loader = env.new_object(
            "dalvik/system/InMemoryDexClassLoader",
            "(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V",
            &[(&buffer).into(), (&parent).into()],
        )?;
        let loader = env.new_global_ref(loader)?;
        let _ = LOADER.set(loader.clone());
        loader
    };
    let name = env.new_string(name)?;
    let class = env
        .call_method(
            loader.as_obj(),
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[(&name).into()],
        )?
        .l()?;
    env.new_global_ref(class)
}
