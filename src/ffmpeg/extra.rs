use ffmpeg_next::ffi::av_buffersrc_write_frame;
use ffmpeg_next::{filter, Error, Frame};

pub trait SourceExtra {
    fn write(&mut self, frame: &Frame) -> Result<(), Error>;
}

impl<'a> SourceExtra for filter::Context<'a> {
    fn write(&mut self, frame: &Frame) -> Result<(), Error> {
        unsafe {
            match av_buffersrc_write_frame(self.as_mut_ptr(), frame.as_ptr() as *mut _) {
                0 => Ok(()),
                e => Err(Error::from(e)),
            }
        }
    }
}

/*
pub trait OptionSettable {
    fn opt_set_str(&mut self, name: &str, value: &str) -> Result<(), Error>;

    fn opt_iter(&self) -> FfmpegOptionIter;

    fn child_iter(&self) -> ChildIter;

    fn print_opts(&self, indent: usize) {
        let indent_str = "    ".repeat(indent);
        let mut opts: Vec<_> = self.opt_iter().collect();
        opts.sort_by(|a, b| a.name().cmp(b.name()));
        for opt in opts {
            let help_str = if let Some(help) = opt.help() {
                help
            } else {
                "No documentation."
            };

            let min_max_str = if (opt.min() == i32::MIN as f64 && opt.max() == i32::MAX as f64)
                || opt.min() == opt.max()
            {
                "".to_string()
            } else {
                format!(" [{}-{}]", opt.min(), opt.max())
            };

            info!(
                "{}'{}' - {}{}",
                &indent_str,
                opt.name(),
                help_str,
                min_max_str
            );
        }

        info!("Children:");
        for (index, child) in self.child_iter().enumerate() {
            info!("{}  Child {}:", &indent_str, index);
            child.print_opts(indent + 1);
        }
    }
}

impl OptionSettable for codec::Context {
    fn opt_set_str(&mut self, name: &str, value: &str) -> Result<(), Error> {
        let name = CString::new(name).unwrap();
        let value = CString::new(value).unwrap();

        match unsafe {
            av_opt_set(
                self.as_mut_ptr() as *mut _,
                name.as_ptr(),
                value.as_ptr(),
                AV_OPT_SEARCH_CHILDREN,
            )
        } {
            0 => Ok(()),
            e => Err(Error::from(e)),
        }
    }

    fn opt_iter(&self) -> FfmpegOptionIter {
        FfmpegOptionIter {
            parent: unsafe { self.as_ptr() as _ },
            opt_ptr: std::ptr::null(),
            phantom: Default::default(),
        }
    }

    fn child_iter(&self) -> ChildIter {
        ChildIter {
            parent: unsafe { self.as_ptr() as _ },
            ptr: std::ptr::null_mut(),
            phantom: Default::default(),
        }
    }
}

pub struct FfmpegOptionIter<'a> {
    parent: *const std::ffi::c_void,
    opt_ptr: *const AVOption,
    phantom: PhantomData<&'a ()>,
}

impl<'a> Iterator for FfmpegOptionIter<'a> {
    type Item = FfmpegOption<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.opt_ptr = unsafe { av_opt_next(self.parent, self.opt_ptr) };

        if self.opt_ptr.is_null() {
            None
        } else {
            Some(FfmpegOption {
                ptr: self.opt_ptr,
                phantom: Default::default(),
            })
        }
    }
}

pub struct FfmpegOption<'a> {
    ptr: *const AVOption,
    phantom: PhantomData<&'a ()>,
}

impl<'a> FfmpegOption<'a> {
    pub fn name(&self) -> &str {
        unsafe {
            CStr::from_ptr((*self.ptr).name)
                .to_str()
                .expect("Name is not UTF-8")
        }
    }

    pub fn help(&self) -> Option<&str> {
        unsafe {
            if (*self.ptr).help.is_null() {
                None
            } else {
                let cstr = CStr::from_ptr((*self.ptr).help);
                if cstr.is_empty() {
                    None
                } else {
                    Some(cstr.to_str().expect("Help is not UTF-8"))
                }
            }
        }
    }

    pub fn min(&self) -> f64 {
        unsafe { (*self.ptr).min }
    }

    pub fn max(&self) -> f64 {
        unsafe { (*self.ptr).max }
    }
}

pub struct ChildIter<'a> {
    parent: *mut std::ffi::c_void,
    ptr: *mut std::ffi::c_void,
    phantom: PhantomData<&'a ()>,
}

impl<'a> Iterator for ChildIter<'a> {
    type Item = OptionChild<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.ptr = unsafe { av_opt_child_next(self.parent, self.ptr) };

        if self.ptr.is_null() {
            None
        } else {
            Some(unsafe {
                OptionChild {
                    ptr: self.ptr,
                    phantom: Default::default(),
                }
            })
        }
    }
}

pub struct OptionChild<'a> {
    ptr: *mut std::ffi::c_void,
    phantom: PhantomData<&'a ()>,
}

impl<'a> OptionSettable for OptionChild<'a> {
    fn opt_set_str(&mut self, name: &str, value: &str) -> Result<(), Error> {
        let name = CString::new(name).unwrap();
        let value = CString::new(value).unwrap();

        match unsafe {
            av_opt_set(
                self.ptr,
                name.as_ptr(),
                value.as_ptr(),
                AV_OPT_SEARCH_CHILDREN,
            )
        } {
            0 => Ok(()),
            e => Err(Error::from(e)),
        }
    }

    fn opt_iter(&self) -> FfmpegOptionIter {
        FfmpegOptionIter {
            parent: self.ptr,
            opt_ptr: std::ptr::null(),
            phantom: Default::default(),
        }
    }

    fn child_iter(&self) -> ChildIter {
        ChildIter {
            parent: self.ptr,
            ptr: std::ptr::null_mut(),
            phantom: Default::default(),
        }
    }
}
 */
