use ffmpeg_next::ffi::{AVClass, __va_list_tag};
use lazy_static::lazy_static;
use std::borrow::Cow;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::mem::transmute;
use std::ops::Shr;
use std::sync::{Arc, Mutex};

lazy_static! {
    static ref BUILDING_STR: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
}

pub fn setup_logging() {
    unsafe { ffmpeg_next::ffi::av_log_set_callback(Some(log_callback)) };
}

pub extern "C" fn log_callback(
    ptr: *mut c_void,
    level: c_int,
    fmt: *const c_char,
    vl: *mut __va_list_tag,
) {
    // NOTE if something is segfaulting, this place has plenty of unsafe use for decoding ffmpeg log messages.

    let avc = if ptr.is_null() {
        None
    } else {
        // This transmutes ptr into a reference to a reference to an AVClass.
        unsafe { (*transmute::<_, *const *const AVClass>(ptr)).as_ref() }
    };

    let item_name = avc.and_then(|avc| avc.item_name).map(|item_name_fn| {
        // This calls a c-function pointer, attempting to get the name of the class sending the log message. This then converts the resulting string pointer into a Cow<str>.
        unsafe { CStr::from_ptr(item_name_fn(ptr)) }.to_string_lossy()
    });

    let item_name = item_name.unwrap_or(Cow::Borrowed("NONE"));

    // This formats the ffmpeg log message the way it expects it to be formatted.
    let printf = match unsafe { vsprintf::vsprintf(fmt, vl) } {
        Ok(s) => s,
        Err(err) => {
            warn!("Error formatting ffmpeg log: {:?}", err);
            return;
        }
    };

    if printf.ends_with('\n') {
        let res = {
            let mut lock = BUILDING_STR.lock().unwrap();

            let res = if let Some(str) = lock.clone() {
                str + &printf[..printf.len() - 1]
            } else {
                printf[..printf.len() - 1].to_string()
            };

            *lock = None;

            res
        };

        let level = level.shr(3i32).clamp(0, 7);

        match level {
            0 => error!("[ffmpeg:PANIC:{}] {}", item_name, res),
            1 => error!("[ffmpeg:FATAL:{}] {}", item_name, res),
            2 => error!("[ffmpeg:ERROR:{}] {}", item_name, res),
            3 => warn!("[ffmpeg: WARN:{}] {}", item_name, res),
            4 => info!("[ffmpeg: INFO:{}] {}", item_name, res),
            5 => debug!("[ffmpeg: VERB:{}] {}", item_name, res),
            6 => debug!("[ffmpeg:DEBUG:{}] {}", item_name, res),
            7 => debug!("[ffmpeg:TRACE:{}] {}", item_name, res),
            _ => unreachable!("Invalid ffmpeg level"),
        }
    } else {
        let mut lock = BUILDING_STR.lock().unwrap();

        let to_store = if let Some(str) = lock.clone() {
            str + &printf
        } else {
            printf
        };

        *lock = Some(to_store);
    }
}
