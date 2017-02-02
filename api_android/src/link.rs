use simplelink;

use jni::objects::{ GlobalRef, JArray, JValue };
use jni::sys::{ jint, jsize };
use jni::JNIEnv;

use rx_tx;

pub struct Link {
    node: simplelink::spec::node::Node,
    obj: GlobalRef
}

pub fn new(callsign: u32, obj: GlobalRef) -> *mut Link {
    Box::into_raw(Box::new(Link {
        node: simplelink::spec::node::new(callsign),
        obj: obj
    }))
}

fn get_frame_data<'a>(env: &'a JNIEnv<'a>, frame: &simplelink::spec::frame::Frame, data: &[u8]) -> Result<(JArray<'a>, JArray<'a>),()> {
    let route_arr = JArray::new_int(env, frame.address_route.len() as jsize).map_err(|_| ())?;
    {
        let mut route_data = route_arr.get_data_int().map_err(|_| ())?;
        
        for i in 0..frame.address_route.len() {
            route_data.get_mut()[i] = frame.address_route[i] as i32;
        }

        route_data.commit().map_err(|_| ())?;
    }

    let data_arr = JArray::new_byte(env, data.len() as jsize).map_err(|_| ())?;
    {
        let mut data_data = data_arr.get_data_byte().map_err(|_| ())?;

        for (i,v) in data.iter().map(|v| *v as i8).enumerate() {
            data_data.get_mut()[i] = v;
        }

        data_data.commit().map_err(|_| ())?;
    }

    Ok((route_arr, data_arr))
}

impl Link {
    pub fn tick(&mut self, env: &JNIEnv, elapsed: usize) -> bool {
        let obj = self.obj.inner();
        let mut rx_tx = rx_tx::new(env, obj);

        let recv_res = self.node.recv(&mut rx_tx,
            |frame, data| {
                if data.len() == 0 {
                    env.call_method(obj, "internal_ack", "(I)V", &[JValue::Int(frame.prn as jint)]).unwrap_or(JValue::Void);
                } else {
                    let (route_arr, data_arr) = match get_frame_data(env, frame, data) {
                        Ok(v) => v,
                        Err(()) => return
                    };

                    env.call_method(obj, "internal_recv", "(I[I[B)V", 
                        &[JValue::Int(frame.prn as jint),
                          JValue::Object(route_arr.into_inner().into()),
                          JValue::Object(data_arr.into_inner().into())]).unwrap_or(JValue::Void);
                }
            },
            |frame, data| {
                let (route_arr, data_arr) = match get_frame_data(env, frame, data) {
                    Ok(v) => v,
                    Err(()) => return
                };

                env.call_method(obj, "internal_observe", "(I[I[B)V", 
                    &[JValue::Int(frame.prn as jint),
                        JValue::Object(route_arr.into_inner().into()),
                        JValue::Object(data_arr.into_inner().into())]).unwrap_or(JValue::Void);
        });

        if let Err(_) = recv_res {
            return false
        }

        let tick_res = self.node.tick(&mut rx_tx, elapsed,
            |frame, _, next_retry| {
                env.call_method(obj, "internal_retry", "(II)V", &[JValue::Int(frame.prn as jint), JValue::Int(next_retry as jint)]).unwrap_or(JValue::Void);
            },
            |frame,_| {
                env.call_method(obj, "internal_expire", "(I)V", &[JValue::Int(frame.prn as jint)]).unwrap_or(JValue::Void);
            });

        if let Err(_) = tick_res {
            return false
        }

        true
    }

    pub fn send<R,D>(&mut self, env: &JNIEnv, route: R, data: D) -> u32
            where R: Iterator<Item=u32>,
                  D: Iterator<Item=u8> {
        let mut rx_tx = rx_tx::new(env, self.obj.inner());

        match self.node.send(data, route, &mut rx_tx) {
            Ok(prn) => prn,
            Err(_) => 0
        }
    }
}