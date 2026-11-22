//! Device arrival and removal from Windows (`CM_Register_Notification` on the HID interface
//! class), so the scanner looks for controllers when one comes or goes instead of enumerating
//! DirectInput every couple of seconds (tens of milliseconds of CPU each time).

use std::ffi::c_void;
use std::time::Duration;

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_NOTIFY_ACTION, CM_NOTIFY_EVENT_DATA, CM_NOTIFY_FILTER, CM_NOTIFY_FILTER_0,
    CM_NOTIFY_FILTER_0_0, CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE, CM_Register_Notification,
    CM_Unregister_Notification, CR_SUCCESS, HCMNOTIFICATION,
};
use windows::Win32::Devices::HumanInterfaceDevice::HidD_GetHidGuid;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForSingleObject};

/// An auto-reset event Windows sets whenever a HID device interface arrives or goes.
pub(super) struct Arrivals {
    event: HANDLE,
    notify: HCMNOTIFICATION,
}

/// Called by Windows on one of its threads; `context` is the event handle.
unsafe extern "system" fn arrived(
    _notify: HCMNOTIFICATION,
    context: *const c_void,
    _action: CM_NOTIFY_ACTION,
    _data: *const CM_NOTIFY_EVENT_DATA,
    _size: u32,
) -> u32 {
    // SAFETY: `context` is the event handle registered with this callback; it stays open until
    // the registration is undone (Drop unregisters first, which waits for running callbacks).
    let _ = unsafe { SetEvent(HANDLE(context as *mut c_void)) };
    0
}

impl Arrivals {
    /// `None` when Windows does not give notifications (the scanner then polls).
    pub(super) fn new() -> Option<Self> {
        // SAFETY: an unnamed auto-reset event, not signaled; closed in Drop.
        let event = unsafe { CreateEventW(None, false, false, None) }.ok()?;
        // SAFETY: plain query of a constant GUID.
        let hid = unsafe { HidD_GetHidGuid() };
        let filter = CM_NOTIFY_FILTER {
            cbSize: size_of::<CM_NOTIFY_FILTER>() as u32,
            FilterType: CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
            u: CM_NOTIFY_FILTER_0 {
                DeviceInterface: CM_NOTIFY_FILTER_0_0 { ClassGuid: hid },
            },
            ..Default::default()
        };
        let mut notify = HCMNOTIFICATION::default();
        // SAFETY: `filter` and `notify` outlive the call; the callback gets the event handle,
        // which lives as long as the registration (see Drop).
        let result = unsafe {
            CM_Register_Notification(
                &filter,
                Some(event.0 as *const c_void),
                Some(arrived),
                &mut notify,
            )
        };
        if result != CR_SUCCESS {
            log::debug!("device notifications unavailable ({result:?}): scanning periodically");
            // SAFETY: the event was created above and is not used elsewhere.
            let _ = unsafe { CloseHandle(event) };
            return None;
        }
        Some(Self { event, notify })
    }

    /// Waits until a device came or went, `at_most` long; true when one did.
    pub(super) fn wait(&self, at_most: Duration) -> bool {
        let ms = u32::try_from(at_most.as_millis()).unwrap_or(u32::MAX);
        // SAFETY: the event is open for the life of `self`.
        unsafe { WaitForSingleObject(self.event, ms) == WAIT_OBJECT_0 }
    }
}

impl Drop for Arrivals {
    fn drop(&mut self) {
        // SAFETY: unregistering waits for callbacks in progress, so the event is closed only
        // after the last one used it.
        unsafe {
            let _ = CM_Unregister_Notification(self.notify);
            let _ = CloseHandle(self.event);
        }
    }
}
