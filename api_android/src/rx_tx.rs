use std::io;
use std::cmp;

use jni::objects::{ JObject, JArray, JValue };
use jni::sys::{ jint };
use jni::errors::{ ErrorKind, Result };
use jni::JNIEnv;

pub struct RxTx<'a> {
    env: &'a JNIEnv<'a>,
    obj: JObject<'a>
}

pub fn new<'a>(env: &'a JNIEnv<'a>, obj: JObject<'a>) -> RxTx<'a> {
    RxTx {
        env: env,
        obj: obj
    }
}

fn get_scratch<'a>(env: &'a JNIEnv<'a>, object: JObject<'a>) -> Result<JObject<'a>> {
    match env.get_field(object, "m_scratch", "[B")? {
        JValue::Object(o) => Ok(o.into()),
        _ => Err(ErrorKind::InvalidArgList.into())
    }
}

impl<'a> io::Read for RxTx<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = match self.env.call_method(self.obj, "internal_fill_read", "(I)I", &[JValue::Int(buf.len() as jint)]) {
            Ok(v) => match v {
                JValue::Int(i) => i,
                _ => -1
            },
            Err(_) => -1
        };

        match read {
            0 => Ok(0),
            -1 => Err(io::Error::from(io::ErrorKind::Other)),
            n => {
                let scratch = match get_scratch(self.env, self.obj) {
                    Ok(s) => s,
                    Err(_) => return Err(io::Error::from(io::ErrorKind::Other))
                };

                let arr = match JArray::from_env(self.env, scratch) {
                    Ok(s) => s,
                    Err(_) => return Err(io::Error::from(io::ErrorKind::Other))
                };

                let slice = match arr.get_data_byte() {
                    Ok(s) => s,
                    Err(_) => return Err(io::Error::from(io::ErrorKind::Other))
                };

                let total_read = cmp::min(n as usize, buf.len());
                for (i, v) in slice.get().iter().take(total_read).map(|v| *v as u8).enumerate() {
                    buf[i] = v;
                }

                Ok(n as usize)
            }
        }
    }
}

impl<'a> io::Write for RxTx<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let read = {
            let scratch = match get_scratch(self.env, self.obj) {
                Ok(s) => s,
                Err(_) => return Err(io::Error::from(io::ErrorKind::Other))
            };

            let arr = match JArray::from_env(self.env, scratch) {
                Ok(s) => s,
                Err(_) => return Err(io::Error::from(io::ErrorKind::Other))
            };

            let mut slice = match arr.get_data_byte() {
                Ok(s) => s,
                Err(_) => return Err(io::Error::from(io::ErrorKind::Other))
            };

            let slice_ref = slice.get_mut();

            let total_read = cmp::min(slice_ref.len(), buf.len());
            for (i, v) in buf.iter().take(total_read).map(|v| ((*v as i32) - 128) as i8).enumerate() {
                slice_ref[i] = v;
            }

            total_read
        };

        match self.env.call_method(self.obj, "internal_flush_write", "(I)I", &[JValue::Int(read as jint)]) {
            Ok(v) => match v {
                JValue::Int(i) => match i {
                    -1 => Err(io::Error::from(io::ErrorKind::Other)),
                    _ => Ok(read)
                },
                _ => Err(io::Error::from(io::ErrorKind::Other))
            },
            _ => Err(io::Error::from(io::ErrorKind::Other))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

