//! Daemon introspection event subscription subsystem.

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

//! # Overview
//!
//! The application can be notified, asynchronously, whenever the internal layout of the server
//! changes. The set of facilities and operations for which notifications are generated are
//! enumerated in [`Facility`] and [`Operation`].
//!
//! The application sets the notification mask using [`::context::Context::subscribe`] and the
//! callback function that will be called whenever a notification occurs using
//! [`::context::Context::set_subscribe_callback`].
//!
//! The mask provided to [`::context::Context::subscribe`] can be created by binary ORing a set of
//! values, either produced with [`Facility::to_interest_mask`], or more simply with the provided
//! constants in the [`subscription_masks`] submodule.
//!
//! The callback will be called with event type information representing the event that caused the
//! callback, detailing *facility* and *operation*, where for instance `Facility::Source` with
//! `Operation::New` indicates that a new source was added.
//!
//! # Example
//!
//! Subscribe (declare interest):
//!
//! ```rust,ignore
//! use pulse::context::subscribe::subscription_masks;
//!
//! let interest = subscription_masks::SINK |
//!     subscription_masks::SOURCE;
//!
//! let op = my_context.subscribe(
//!     interest,   // Our interest mask
//!     None        // We won't bother with a success callback in this example
//! );
//! ```
//!
//! [`Facility`]: enum.Facility.html
//! [`Operation`]: enum.Operation.html
//! [`Facility::to_interest_mask`]: enum.Facility.html#method.to_interest_mask
//! [`::context::Context::subscribe`]: ../struct.Context.html#method.subscribe
//! [`::context::Context::set_subscribe_callback`]: ../struct.Context.html#method.set_subscribe_callback
//! [`subscription_masks`]: subscription_masks/index.html

use capi;
use std::os::raw::c_void;
use super::{ContextInternal, Context};

pub use capi::context::subscribe::pa_subscription_event_type_t as EventType;
pub use capi::PA_SUBSCRIPTION_EVENT_FACILITY_MASK as FACILITY_MASK;
pub use capi::PA_SUBSCRIPTION_EVENT_TYPE_MASK as OPERATION_MASK;

/// A set of facility masks, passed to
/// [`Context::subscribe`](../struct.Context.html#method.subscribe). Convert a
/// [`Facility`](enum.Facility.html) to a mask with [`facility_to_mask`](fn.facility_to_mask.html).
pub type InterestMaskSet = capi::context::subscribe::pa_subscription_mask_t;

/// A set of masks used for expressing which facilities you are interested in when subscribing.
pub mod subscription_masks {
    use capi;
    use super::InterestMaskSet;

    pub const NULL: InterestMaskSet = capi::PA_SUBSCRIPTION_MASK_NULL;
    pub const SINK: InterestMaskSet = capi::PA_SUBSCRIPTION_MASK_SINK;
    pub const SOURCE: InterestMaskSet = capi::PA_SUBSCRIPTION_MASK_SOURCE;
    pub const SINK_INPUT: InterestMaskSet = capi::PA_SUBSCRIPTION_MASK_SINK_INPUT;
    pub const SOURCE_OUTPUT: InterestMaskSet = capi::PA_SUBSCRIPTION_MASK_SOURCE_OUTPUT;
    pub const MODULE: InterestMaskSet = capi::PA_SUBSCRIPTION_MASK_MODULE;
    pub const CLIENT: InterestMaskSet = capi::PA_SUBSCRIPTION_MASK_CLIENT;
    pub const SAMPLE_CACHE: InterestMaskSet = capi::PA_SUBSCRIPTION_MASK_SAMPLE_CACHE;
    pub const SERVER: InterestMaskSet = capi::PA_SUBSCRIPTION_MASK_SERVER;
    pub const MASK_CARD: InterestMaskSet = capi::PA_SUBSCRIPTION_MASK_CARD;
    pub const ALL: InterestMaskSet = capi::PA_SUBSCRIPTION_MASK_ALL;
}

/// Facility component of an event.
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Facility {
    Sink = 0,
    Source = 1,
    SinkInput = 2,
    SourceOutput = 3,
    Module = 4,
    Client = 5,
    SampleCache = 6,
    /// Global server change, only occurring with
    /// [`Operation::Changed`](enum.Operation.html#Changed.v).
    Server = 7,
    /* NOTE: value '8' previously assigned, obsoleted */
    Card = 9,
}

/// Operation component of an event.
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Operation {
    /// A new object was created
    New = 0,
    /// A property of the object was modified
    Changed = 0x10,
    /// An object was removed
    Removed = 0x20,
}

impl Facility {
    fn from_int(value: u32) -> Option<Facility> {
        match value {
            0 => Some(Facility::Sink),
            1 => Some(Facility::Source),
            2 => Some(Facility::SinkInput),
            3 => Some(Facility::SourceOutput),
            4 => Some(Facility::Module),
            5 => Some(Facility::Client),
            6 => Some(Facility::SampleCache),
            7 => Some(Facility::Server),
            /* NOTE: value '8' previously assigned, obsoleted */
            9 => Some(Facility::Card),
            _ => None,
        }
    }

    /// Convert to an interest mask
    pub fn to_interest_mask(self) -> InterestMaskSet {
        1u32 << (self as u32)
    }
}

impl Operation {
    fn from_int(value: u32) -> Option<Operation> {
        match value {
            0 => Some(Operation::New),
            0x10 => Some(Operation::Changed),
            0x20 => Some(Operation::Removed),
            _ => None,
        }
    }
}

/// Extract facility from `EventType` value
fn get_facility(value: EventType) -> Option<Facility> {
    Facility::from_int((value & FACILITY_MASK) as u32)
}

/// Extract operation from `EventType` value
fn get_operation(value: EventType) -> Option<Operation> {
    Operation::from_int((value & OPERATION_MASK) as u32)
}

pub(super) type Callback = ::callbacks::MultiUseCallback<FnMut(Option<Facility>, Option<Operation>,
    u32), extern "C" fn(*mut ContextInternal, EventType, u32, *mut c_void)>;

impl Context {
    /// Enable event notification.
    /// The `mask` parameter is used to specify which facilities you are interested in being
    /// modified about. Use [`set_subscribe_callback`](#method.set_subscribe_callback) to set the
    /// actual callback that will be called when an event occurs.
    ///
    /// The callback must accept a `bool`, which indicates success.
    pub fn subscribe<F>(&mut self, mask: InterestMaskSet, callback: F) -> ::operation::Operation
        where F: FnMut(bool) + 'static
    {
        let cb_data: *mut c_void = {
            // WARNING: Type must be explicit here, else compiles but seg faults :/
            let boxed: *mut Box<FnMut(bool)> = Box::into_raw(Box::new(Box::new(callback)));
            boxed as *mut c_void
        };
        let ptr = unsafe { capi::pa_context_subscribe(self.ptr, mask, Some(super::success_cb_proxy),
            cb_data) };
        assert!(!ptr.is_null());
        ::operation::Operation::from_raw(ptr)
    }

    /// Set the context specific call back function that is called whenever a subscribed-to event
    /// occurs. Use [`subscribe`](#method.subscribe) to set the facilities you are interested in
    /// receiving notifications for, and thus to start receiving notifications with the callback set
    /// here.
    ///
    /// The callback must take three parameters. The first two are the facility and operation
    /// components of the event type respectively (the underlying C API provides this information
    /// combined into a single integer, here we extract the two component parts for you); these are
    /// wrapped in `Option` wrappers should the given values ever not map to the enum variants, but
    /// it's probably safe to always just `unwrap()` them). The third parameter is an associated
    /// index value.
    pub fn set_subscribe_callback(&mut self,
        callback: Option<Box<FnMut(Option<Facility>, Option<Operation>, u32) + 'static>>)
    {
        let saved = &mut self.cb_ptrs.subscribe;
        *saved = Callback::new(callback);
        let (cb_fn, cb_data) = saved.get_capi_params(cb_proxy);
        unsafe { capi::pa_context_set_subscribe_callback(self.ptr, cb_fn, cb_data); }
    }
}

/// Proxy for callbacks.
/// Warning: This is for multi-use cases! It does **not** destroy the actual closure callback, which
/// must be accomplished separately to avoid a memory leak.
extern "C"
fn cb_proxy(_: *mut ContextInternal, et: EventType, index: u32, userdata: *mut c_void) {
    assert!(!userdata.is_null());
    // Note, does NOT destroy closure callback after use - only handles pointer
    let callback = unsafe { &mut *(userdata as *mut Box<FnMut(Option<Facility>,
        Option<Operation>, u32)>) };
    let facility = get_facility(et);
    let operation = get_operation(et);
    callback(facility, operation, index);
}
