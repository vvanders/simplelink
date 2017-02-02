extern crate jni;
#[macro_use]
extern crate log;
extern crate fern;
extern crate simplelink;

mod link;
mod rx_tx;

use jni::objects::*;
use jni::sys::*;

static mut ENV: Option<*mut jni::sys::JNIEnv> = None;
static mut LOG_LOCK: bool = false;

unsafe fn logcat<T>(env: &jni::JNIEnv, level: &log::LogLevel, input: T) where T: std::string::ToString {
    if LOG_LOCK {
        return
    }

    LOG_LOCK = true;

    {
        let func = match *level {
            log::LogLevel::Trace => "v",
            log::LogLevel::Info => "i",
            log::LogLevel::Debug => "d",
            log::LogLevel::Warn => "w",
            log::LogLevel::Error => "e"
        };

        let cls = LocalRef::from_env(env, env.find_class("android/util/Log").unwrap());
        let logcat = env.get_static_method_id(*cls.as_ref(), func, "(Ljava/lang/String;Ljava/lang/String;)I").unwrap();

        let tag = LocalRef::from_env(env, env.new_string("Rust").unwrap());
        let log = LocalRef::from_env(env, env.new_string(input.to_string()).unwrap());
        env.call_static_method_unsafe(*cls.as_ref(),
            logcat,
            jni::signature::JavaType::Primitive(jni::signature::Primitive::Int),
            &[JValue::Object(**tag.as_ref()), JValue::Object(**log.as_ref())]).unwrap();
    }

    LOG_LOCK = false
}

//Sets the global env for logging
unsafe fn set_env(env: &jni::JNIEnv) {
    ENV = Some(env.inner());
}

unsafe fn get_link(env: &jni::JNIEnv, object: JObject) -> *mut link::Link {
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
pub unsafe extern "C" fn Java_vvanders_com_simplelink_SimpleLink_internal_1init(env: jni::JNIEnv, object: JObject, callsign: JString) -> jboolean {
    set_env(&env);

    use simplelink::spec::address;

    let callsign: String = env.get_string(callsign).expect("Failed").into();

    if callsign.len() > 7 {
        return JNI_FALSE;
    }

    let mut callsign_scratch: [char; 7] = ['0'; 7];

    for (i, c) in callsign.chars().enumerate().take(7) {
        callsign_scratch[i] = c;
    }

    let callsign_id = match address::encode(callsign_scratch) {
        Some(c) => c,
        None => return JNI_FALSE
    };

    let obj_ref = GlobalRef::from(&env, &object).unwrap();

    let link = link::new(callsign_id, obj_ref);

    match env.set_field(object, "m_link", "J", JValue::Long(link as i64)) {
        Ok(()) => (),
        Err(_) => return JNI_FALSE
    }

    JNI_TRUE
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_vvanders_com_simplelink_SimpleLink_tick(env: jni::JNIEnv, object: JObject, elapsed_ms: jint) -> jboolean {
    set_env(&env);

    let link = get_link(&env, object);

    if (*link).tick(&env, elapsed_ms as usize) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_vvanders_com_simplelink_SimpleLink_send(env: jni::JNIEnv, object: JObject, route: JObject, data: JObject) -> jint {
    set_env(&env);

    let data_array = JArray::from_env(&env, data).unwrap();
    let data_bytes = data_array.get_data_byte().unwrap();

    let route_array = JArray::from_env(&env, route).unwrap();
    let route_data = route_array.get_data_int().unwrap();

    let link = get_link(&env, object);

    (*link).send(&env, route_data.get().iter().map(|v| *v as u32), data_bytes.get().iter().map(|v| *v as u8)) as jint
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_vvanders_com_simplelink_SimpleLink_decode_1addr(env: jni::JNIEnv, _object: JObject, addr: jint) -> jstring {
    let translated = simplelink::spec::address::format_addr(addr as u32);

    env.new_string(translated).unwrap().into_inner()
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_vvanders_com_simplelink_SimpleLink_encode_1addr(env: jni::JNIEnv, _object: JObject, addr: JString) -> jint {
    let addr_str: String = env.get_string(addr).unwrap().into();

    if addr_str.len() > 7 {
        return 0
    }

    let mut translated: [char; 7] = ['0'; 7];

    for (i, c) in addr_str.chars().enumerate().take(7) {
        translated[i] = c;
    }
   
    simplelink::spec::address::encode(translated).unwrap_or(0) as jint
}