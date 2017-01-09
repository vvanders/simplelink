extern crate jni;
#[macro_use]
extern crate log;
extern crate fern;
extern crate slink;
extern crate simplelink;

use jni::objects::*;
use jni::sys::*;

static mut ENV: Option<*mut jni::sys::JNIEnv> = None;
static mut LOG_LOCK: bool = false;

unsafe fn logcat<T>(env: &jni::JNIEnv, level: &log::LogLevel, input: T) where T: std::string::ToString {
    if LOG_LOCK {
        return
    }

    LOG_LOCK = true;

    let func = match *level {
        log::LogLevel::Trace => "v",
        log::LogLevel::Info => "i",
        log::LogLevel::Debug => "d",
        log::LogLevel::Warn => "w",
        log::LogLevel::Error => "e"
    };

    let cls = env.find_class("android/util/Log").unwrap();
    let logcat = env.get_static_method_id(cls, func, "(Ljava/lang/String;Ljava/lang/String;)I").unwrap();

    let tag = env.new_string("Rust").unwrap();
    let log = env.new_string(input.to_string()).unwrap();
    env.call_static_method_unsafe("android/util/Log",
        logcat,
        jni::signature::JavaType::Primitive(jni::signature::Primitive::Int),
        &[JValue::Object(*tag), JValue::Object(*log)]).unwrap();

    LOG_LOCK = false
}

//Sets the global env for logging
unsafe fn set_env(env: &jni::JNIEnv) {
    ENV = Some(env.inner());
}

unsafe fn get_link(env: &jni::JNIEnv, object: JObject) -> *mut slink::Link {
    match env.get_field(object, "m_link", "J") {
        Ok(v) => match v {
            JValue::Long(f) => {
                #[cfg(target_os = "android")]
                {
                    std::mem::transmute(f as i32)
                }
                #[cfg(not(target_os = "android"))]
                {
                    std::mem::transmute(f)
                }
            },
            _ => std::ptr::null_mut()
        },
        Err(_) => std::ptr::null_mut()
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_vvanders_com_simplelink_SimpleLink_static_1init(env: jni::JNIEnv, _class: JClass) {
    set_env(&env);

    simplelink::util::init_log_callback(log::LogLevelFilter::Trace, false,
        |msg, level, _location| {
            logcat(&jni::JNIEnv::from(ENV.unwrap()), level, msg);
        }
    );
}


#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_vvanders_com_simplelink_SimpleLink_init(env: jni::JNIEnv, object: JObject, callsign: JString) -> jboolean {
    set_env(&env);

    use simplelink::spec::address;

    let callsign: String = env.get_string(callsign).expect("Failed").into();

    if callsign.len() > 7 {
        return 0;
    }

    let mut callsign_scratch: [char; 7] = ['0'; 7];

    for (i, c) in callsign.chars().enumerate().take(7) {
        callsign_scratch[i] = c;
    }

    let callsign_id = match address::encode(callsign_scratch) {
        Some(c) => c,
        None => return 0
    };

    let link = slink::new_nolog(callsign_id);

    match env.set_field(object, "m_link", "J", JValue::Long(link as i64)) {
        Ok(()) => (),
        Err(_) => return 0
    }

    1
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_vvanders_com_simplelink_SimpleLink_open_1loopback(env: jni::JNIEnv, object: JObject) -> jboolean {
    set_env(&env);

    let link = get_link(&env, object);

    match slink::open_loopback(link) {
        true => 1,
        false => 0
    }
}