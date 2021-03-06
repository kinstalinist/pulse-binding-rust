//! Utility functions for handling a stream or sink format.

// This file is part of the PulseAudio Rust language binding.
//
// Copyright (c) 2017 Lyndon Brown
//
// This library is free software; you can redistribute it and/or modify it under the terms of the
// GNU Lesser General Public License as published by the Free Software Foundation; either version
// 2.1 of the License, or (at your option) any later version.
//
// This library is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without
// even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public License along with this library;
// if not, see <http://www.gnu.org/licenses/>.

//! # Note
//!
//! Clients using an [`Info`] structure must remember to at least set the encoding attribute, which
//! can be done through the [`set_encoding`] method.
//!
//! [`Info`]: struct.Info.html
//! [`set_encoding`]: struct.Info.html#method.set_encoding

use std;
use capi;
use std::os::raw::{c_char, c_void};
use std::ffi::{CStr, CString};
use std::ptr::{null, null_mut};
use error::PAErr;

pub use capi::pa_prop_type_t as PropType;

/// Represents the type of encoding used in a stream or accepted by a sink.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum Encoding {
    /// Any encoding format, PCM or compressed.
    Any,
    /// Any PCM format.
    PCM,
    /// AC3 data encapsulated in IEC 61937 header/padding.
    AC3_IEC61937,
    /// EAC3 data encapsulated in IEC 61937 header/padding.
    EAC3_IEC61937,
    /// MPEG-1 or MPEG-2 (Part 3, not AAC) data encapsulated in IEC 61937 header/padding.
    MPEG_IEC61937,
    /// DTS data encapsulated in IEC 61937 header/padding.
    DTS_IEC61937,
    /// MPEG-2 AAC data encapsulated in IEC 61937 header/padding.
    MPEG2_AAC_IEC61937,

    /// Represents an invalid encoding.
    Invalid = -1,
}

impl From<Encoding> for capi::pa_encoding_t {
    fn from(e: Encoding) -> Self {
        unsafe { std::mem::transmute(e) }
    }
}

impl From<capi::pa_encoding_t> for Encoding {
    fn from(e: capi::pa_encoding_t) -> Self {
        unsafe { std::mem::transmute(e) }
    }
}

impl Default for Encoding {
    fn default() -> Self {
        Encoding::Invalid
    }
}

/// Represents the format of data provided in a stream or processed by a sink.
pub struct Info {
    /// The actual C object
    pub(crate) ptr: *mut InfoInternal,
    /// Wrapped property list pointer
    properties: ::proplist::Proplist,
    /// Used to avoid freeing the internal object when used as a weak wrapper in callbacks
    weak: bool,
}

/// The raw C structure (with documentation)
#[repr(C)]
pub(crate) struct InfoInternal {
    /// The encoding used for the format.
    pub encoding: Encoding,
    /// Additional encoding-specific properties such as sample rate, bitrate, etc.
    pub list: *mut ::proplist::ProplistInternal,
}

impl From<capi::pa_format_info> for InfoInternal {
    fn from(i: capi::pa_format_info) -> Self {
        unsafe { std::mem::transmute(i) }
    }
}

impl From<InfoInternal> for capi::pa_format_info {
    fn from(i: InfoInternal) -> Self {
        unsafe { std::mem::transmute(i) }
    }
}

impl std::fmt::Debug for Info {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Info {{ encoding: {:?}, properties: {:?} }}", self.get_encoding(),
            *self.get_properties())
    }
}

impl Encoding {
    /// Returns a printable string representing the given encoding type.
    pub fn to_string(e: Self) -> Option<String> {
        let ptr = unsafe { capi::pa_encoding_to_string(e.into()) };
        if ptr.is_null() {
            return None;
        }
        Some(unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() })
    }

    /// Converts a string of the form returned by [`to_string`](#method.to_string) back to an
    /// `Encoding`.
    ///
    /// Only included if 'pa_encoding_from_string' feature enabled, because symbol seems to be
    /// missing (at least on Debian Sid).
    #[cfg(feature = "pa_encoding_from_string")]
    pub fn from_string(encoding: &str) -> Self {
        // Warning: New CStrings will be immediately freed if not bound to a variable, leading to
        // as_ptr() giving dangling pointers!
        let c_enc = CString::new(encoding.clone()).unwrap();
        unsafe { capi::pa_encoding_from_string(c_enc.as_ptr()) }
    }
}

impl Info {
    /// Allocates a new `Info` structure.
    ///
    /// Clients must initialise at least the encoding field themselves. Returns `None` on failure.
    pub fn new() -> Option<Self> {
        let ptr = unsafe { capi::pa_format_info_new() };
        if ptr.is_null() {
            return None;
        }
        Some(Self::from_raw(unsafe { std::mem::transmute(ptr) }))
    }

    /// Parse a human-readable string of the form generated by [`print`](#method.print) into an
    /// `Info` structure.
    ///
    /// Returns `None` on failure.
    pub fn new_from_string(s: &str) -> Option<Self> {
        // Warning: New CStrings will be immediately freed if not bound to a variable, leading to
        // as_ptr() giving dangling pointers!
        let c_str = CString::new(s.clone()).unwrap();
        let ptr = unsafe { capi::pa_format_info_from_string(c_str.as_ptr()) };
        if ptr.is_null() {
            return None;
        }
        Some(Self::from_raw(unsafe { std::mem::transmute(ptr) }))
    }

    /// Utility function to take a [`::sample::Spec`] and generate the corresponding `Info`.
    ///
    /// Note that if you want the server to choose some of the stream parameters, for example the
    /// sample rate, so that they match the device parameters, then you shouldn't use this function.
    /// In order to allow the server to choose a parameter value, that parameter must be left
    /// unspecified in the `Info` object, and this function always specifies all parameters. An
    /// exception is the channel map: if you pass `None` for the channel map, then the channel map
    /// will be left unspecified, allowing the server to choose it.
    ///
    /// Returns `None` on failure.
    ///
    /// [`::sample::Spec`]: ../sample/struct.Spec.html
    pub fn new_from_sample_spec(ss: &::sample::Spec, map: Option<&::channelmap::Map>
        ) -> Option<Self>
    {
        let p_map: *const capi::pa_channel_map = match map {
            Some(map) => unsafe { std::mem::transmute(map) },
            None => null::<capi::pa_channel_map>(),
        };
        let ptr = unsafe { capi::pa_format_info_from_sample_spec(std::mem::transmute(ss), p_map) };
        if ptr.is_null() {
            return None;
        }
        Some(Self::from_raw(unsafe { std::mem::transmute(ptr) }))
    }

    /// Create a new `Info` from an existing [`InfoInternal`](struct.InfoInternal.html) pointer.
    pub(crate) fn from_raw(ptr: *mut InfoInternal) -> Self {
        assert_eq!(false, ptr.is_null());
        unsafe {
            Self {
                ptr: ptr,
                // Note, yes, this should be the weak version, the 'free' function for a format info
                // object free's its own proplist!
                properties: ::proplist::Proplist::from_raw_weak((*ptr).list),
                weak: false,
            }
        }
    }

    /// Create a new `Info` from an existing [`InfoInternal`](struct.InfoInternal.html) pointer.
    /// This is the 'weak' version, which avoids destroying the internal object when dropped.
    pub(crate) fn from_raw_weak(ptr: *mut InfoInternal) -> Self {
        assert_eq!(false, ptr.is_null());
        unsafe {
            Self {
                ptr: ptr,
                properties: ::proplist::Proplist::from_raw_weak((*ptr).list),
                weak: true,
            }
        }
    }

    /// Returns whether the `Info` structure is valid.
    pub fn is_valid(&self) -> bool {
        unsafe { capi::pa_format_info_valid(std::mem::transmute(&self.ptr)) != 0 }
    }

    /// Returns whether the `Info` structure represents a PCM (i.e. uncompressed data) format.
    pub fn is_pcm(&self) -> bool {
        unsafe { capi::pa_format_info_is_pcm(std::mem::transmute(&self.ptr)) != 0 }
    }

    /// Returns whether the format represented by self is a subset of the format represented by
    /// `with`. This means that `with` must have all the fields that self does, but the reverse need
    /// not be true. This is typically expected to be used to check if a stream's format is
    /// compatible with a given sink. In such a case, self would be the sink's format and `with`
    /// would be the streams.
    pub fn is_compatible_with(&self, with: &Self) -> bool {
        unsafe { capi::pa_format_info_is_compatible(std::mem::transmute(&self.ptr),
            std::mem::transmute(&with.ptr)) != 0 }
    }

    /// Return a human-readable string representing the given format.
    pub fn print(&self) -> String {
        const PRINT_MAX: usize = capi::PA_FORMAT_INFO_SNPRINT_MAX;
        let mut tmp = Vec::with_capacity(PRINT_MAX);
        unsafe {
            capi::pa_format_info_snprint(tmp.as_mut_ptr(), PRINT_MAX,
                std::mem::transmute(&self.ptr));
            CStr::from_ptr(tmp.as_mut_ptr()).to_string_lossy().into_owned()
        }
    }

    /// Utility function to generate a [`::sample::Spec`] and [`::channelmap::Map`] corresponding to
    /// a given `Info`.
    ///
    /// The conversion for PCM formats is straight-forward. For non-PCM formats, if there is a fixed
    /// size-time conversion (i.e. all IEC61937-encapsulated formats), a "fake" sample spec whose
    /// size-time conversion corresponds to this format is provided and the channel map argument is
    /// ignored. For formats with variable size-time conversion, this function will fail.
    ///
    /// [`::sample::Spec`]: ../sample/struct.Spec.html
    /// [`::channelmap::Map`]: ../channelmap/struct.Map.html
    pub fn to_sample_spec(&self, ss: &mut ::sample::Spec, map: &mut ::channelmap::Map
        ) -> Result<(), PAErr>
    {
        match unsafe { capi::pa_format_info_to_sample_spec(std::mem::transmute(&self.ptr),
            std::mem::transmute(ss), std::mem::transmute(map)) }
        {
            0 => Ok(()),
            e => Err(PAErr(e)),
        }
    }

    /// Get the encoding.
    pub fn get_encoding(&self) -> Encoding {
        unsafe { (*self.ptr).encoding }
    }

    /// Set the encoding attribute.
    pub fn set_encoding(&mut self, encoding: Encoding) {
        unsafe { (*self.ptr).encoding = encoding };
    }

    /// Get an immutable reference to the property list.
    pub fn get_properties(&self) -> &::proplist::Proplist {
        &self.properties
    }

    /// Get a mutable reference to the property list.
    pub fn get_properties_mut(&mut self) -> &mut ::proplist::Proplist {
        &mut self.properties
    }

    /// Gets the type of property key.
    pub fn get_prop_type(&self, key: &str) -> PropType {
        // Warning: New CStrings will be immediately freed if not bound to a variable, leading to
        // as_ptr() giving dangling pointers!
        let c_key = CString::new(key.clone()).unwrap();
        unsafe { capi::pa_format_info_get_prop_type(std::mem::transmute(&self.ptr), c_key.as_ptr()) }
    }

    /// Gets an integer property.
    pub fn get_prop_int(&self, key: &str) -> Result<i32, PAErr> {
        // Warning: New CStrings will be immediately freed if not bound to a variable, leading to
        // as_ptr() giving dangling pointers!
        let mut i: i32 = 0;
        let c_key = CString::new(key.clone()).unwrap();
        match unsafe { capi::pa_format_info_get_prop_int(std::mem::transmute(&self.ptr),
            c_key.as_ptr(), &mut i) }
        {
            0 => Ok(i),
            e => Err(PAErr(e)),
        }
    }

    /// Gets an integer range property. On success, returns min-max tuple.
    pub fn get_prop_int_range(&self, key: &str) -> Result<(i32, i32), PAErr> {
        // Warning: New CStrings will be immediately freed if not bound to a variable, leading to
        // as_ptr() giving dangling pointers!
        let mut min: i32 = 0;
        let mut max: i32 = 0;
        let c_key = CString::new(key.clone()).unwrap();
        match unsafe { capi::pa_format_info_get_prop_int_range(std::mem::transmute(&self.ptr),
            c_key.as_ptr(), &mut min, &mut max) }
        {
            0 => Ok((min, max)),
            e => Err(PAErr(e)),
        }
    }

    /// Gets an integer array property.
    ///
    /// Returns `None` on error.
    pub fn get_prop_int_array(&self, key: &str) -> Option<Vec<i32>> {
        // Warning: New CStrings will be immediately freed if not bound to a variable, leading to
        // as_ptr() giving dangling pointers!
        let c_key = CString::new(key.clone()).unwrap();
        let mut count: i32 = 0;
        let mut p_ints = null_mut::<i32>();
        let result = unsafe { capi::pa_format_info_get_prop_int_array(
            std::mem::transmute(&self.ptr), c_key.as_ptr(), &mut p_ints, &mut count) };
        if result != 0 {
            return None;
        }
        // Clone each int in the array
        let mut values: Vec<i32> = Vec::with_capacity(count as usize);
        for i in 0..count {
            values.push(unsafe { *p_ints.offset(i as isize) });
        }
        // Free the PA allocated array
        unsafe { capi::pa_xfree(p_ints as *mut c_void) };
        // Return vector of ints
        Some(values)
    }

    /// Gets a string property.
    pub fn get_prop_string(&self, key: &str) -> Option<String> {
        // Warning: New CStrings will be immediately freed if not bound to a variable, leading to
        // as_ptr() giving dangling pointers!
        let c_key = CString::new(key.clone()).unwrap();
        let mut p_str = null_mut::<c_char>();
        let result = unsafe { capi::pa_format_info_get_prop_string(std::mem::transmute(&self.ptr),
            c_key.as_ptr(), &mut p_str) };
        if result != 0 || p_str.is_null() {
            return None;
        }
        unsafe {
            let ret = Some(CStr::from_ptr(p_str).to_string_lossy().into_owned());
            capi::pa_xfree(p_str as *mut c_void);
            ret
        }
    }

    /// Gets a string array property.
    pub fn get_prop_string_array(&self, key: &str) -> Option<Vec<String>> {
        // Warning: New CStrings will be immediately freed if not bound to a variable, leading to
        // as_ptr() giving dangling pointers!
        let c_key = CString::new(key.clone()).unwrap();
        let mut count: i32 = 0;
        let mut pp_str = null_mut::<*mut c_char>();
        let result = unsafe { capi::pa_format_info_get_prop_string_array(
            std::mem::transmute(&self.ptr), c_key.as_ptr(), &mut pp_str, &mut count) };
        if result != 0 || pp_str.is_null() {
            return None;
        }
        // Clone each string in the array to owned String
        let mut values: Vec<String> = Vec::with_capacity(count as usize);
        for i in 0..count {
            let p_str = unsafe { *pp_str.offset(i as isize) };
            if !p_str.is_null() {
                values.push(unsafe { CStr::from_ptr(p_str).to_string_lossy().into_owned() });
            }
        }
        // Free all PA internally allocated strings
        unsafe { capi::pa_format_info_free_string_array(pp_str, count) };
        // Return vector of Strings
        Some(values)
    }

    /// Sets an integer property.
    pub fn set_prop_int(&mut self, key: &str, value: i32) {
        // Warning: New CStrings will be immediately freed if not bound to a variable, leading to
        // as_ptr() giving dangling pointers!
        let c_key = CString::new(key.clone()).unwrap();
        unsafe { capi::pa_format_info_set_prop_int(std::mem::transmute(&self.ptr), c_key.as_ptr(),
            value); }
    }

    /// Sets a property with a list of integer values.
    pub fn set_prop_int_array(&mut self, key: &str, values: &[i32]) {
        // Warning: New CStrings will be immediately freed if not bound to a variable, leading to
        // as_ptr() giving dangling pointers!
        let c_key = CString::new(key.clone()).unwrap();
        unsafe { capi::pa_format_info_set_prop_int_array(std::mem::transmute(&self.ptr),
            c_key.as_ptr(), values.as_ptr(), values.len() as i32); }
    }

    /// Sets a property which can have any value in a given integer range.
    pub fn set_prop_int_range(&mut self, key: &str, min: i32, max: i32) {
        // Warning: New CStrings will be immediately freed if not bound to a variable, leading to
        // as_ptr() giving dangling pointers!
        let c_key = CString::new(key.clone()).unwrap();
        unsafe { capi::pa_format_info_set_prop_int_range(std::mem::transmute(&self.ptr),
            c_key.as_ptr(), min, max); }
    }

    /// Sets a string property.
    pub fn set_prop_string(&mut self, key: &str, value: &str) {
        // Warning: New CStrings will be immediately freed if not bound to a variable, leading to
        // as_ptr() giving dangling pointers!
        let c_key = CString::new(key.clone()).unwrap();
        let c_value = CString::new(value.clone()).unwrap();
        unsafe { capi::pa_format_info_set_prop_string(std::mem::transmute(&self.ptr),
            c_key.as_ptr(), c_value.as_ptr()); }
    }

    /// Sets a property with a list of string values.
    pub fn set_prop_string_array(&mut self, key: &str, values: &[&str]) {
        // Warning: New CStrings will be immediately freed if not bound to a variable, leading to
        // as_ptr() giving dangling pointers!
        let c_key = CString::new(key.clone()).unwrap();
        let mut c_values: Vec<CString> = Vec::with_capacity(values.len());
        for v in values {
            c_values.push(CString::new(v.clone()).unwrap());
        }

        // Capture array of pointers to the above CString values
        let mut c_value_ptrs: Vec<*const c_char> = Vec::with_capacity(c_values.len());
        for v in c_values {
            c_value_ptrs.push(v.as_ptr());
        }
        unsafe {
            capi::pa_format_info_set_prop_string_array(std::mem::transmute(&self.ptr),
                c_key.as_ptr(), c_value_ptrs.as_ptr(), c_value_ptrs.len() as i32);
        }
    }

    /// Convenience method to set the sample format as a property.
    ///
    /// Note for PCM: If the sample format is left unspecified in the `Info` object, then the server
    /// will select the stream sample format. In that case the stream sample format will most likely
    /// match the device sample format, meaning that sample format conversion will be avoided.
    pub fn set_sample_format(&mut self, sf: ::sample::Format) {
        unsafe { capi::pa_format_info_set_sample_format(std::mem::transmute(&self.ptr),
            sf.into()); }
    }

    /// Convenience method to set the sampling rate as a property.
    ///
    /// Note for PCM: If the sample rate is left unspecified in the `Info` object, then the server
    /// will select the stream sample rate. In that case the stream sample rate will most likely
    /// match the device sample rate, meaning that sample rate conversion will be avoided.
    pub fn set_rate(&mut self, rate: i32) {
        unsafe { capi::pa_format_info_set_rate(std::mem::transmute(&self.ptr), rate) }
    }

    /// Convenience method to set the number of channels as a property.
    ///
    /// Note for PCM: If the channel count is left unspecified in the `Info` object, then the server
    /// will select the stream channel count. In that case the stream channel count will most likely
    /// match the device channel count, meaning that up/downmixing will be avoided.
    pub fn set_channels(&mut self, channels: u32) {
        debug_assert!(channels <= std::i32::MAX as u32);
        unsafe { capi::pa_format_info_set_channels(std::mem::transmute(&self.ptr), channels as i32) }
    }

    /// Convenience method to set the channel map as a property.
    ///
    /// Note for PCM: If the channel map is left unspecified in the `Info` object, then the server
    /// will select the stream channel map. In that case the stream channel map will most likely
    /// match the device channel map, meaning that remixing will be avoided.
    pub fn set_channel_map(&mut self, map: &::channelmap::Map) {
        unsafe { capi::pa_format_info_set_channel_map(std::mem::transmute(&self.ptr),
            std::mem::transmute(map)) }
    }
}

impl Drop for Info {
    fn drop(&mut self) {
        if !self.weak {
            unsafe { capi::pa_format_info_free(std::mem::transmute(&self.ptr)) };
        }
    }
}

impl Clone for Info {
    /// Returns a new `Info` struct and representing the same format. If this is called on a 'weak'
    /// instance, a non-weak object is returned.
    fn clone(&self) -> Self {
        let ptr = unsafe { capi::pa_format_info_copy(std::mem::transmute(&self.ptr)) };
        assert_eq!(false, ptr.is_null());
        Self::from_raw(unsafe { std::mem::transmute(ptr) })
    }
}
