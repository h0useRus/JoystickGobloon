//! vJoy device configuration as "Configure vJoy" shows it, with vJoyConfig's command-line
//! syntax (`<id> -f -a X Y ... -b 32 -p 1`) for the elevated `configure` step.

use std::path::Path;

/// Axis names in slot order (1 = X ... 8 = Slider 2), as vJoyConfig spells them.
pub const AXES: [&str; 8] = ["X", "Y", "Z", "Rx", "Ry", "Rz", "Sl0", "Sl1"];
/// Force feedback effects as vJoyConfig spells them.
pub const EFFECTS: [&str; 11] = [
    "Const", "Ramp", "Sq", "Sine", "Tr", "StUp", "StDn", "Spr", "Dm", "Inr", "Fric",
];
pub const MAX_DEVICES: u32 = 16;
pub const MAX_BUTTONS: u32 = 128;
pub const MAX_HATS: u32 = 4;

/// One vJoy device as "Configure vJoy" shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceConfig {
    pub id: u32,
    pub axes: [bool; 8],
    pub buttons: u32,
    pub hats: u32,
    /// 4-way (discrete) hats instead of continuous ones.
    pub discrete_hats: bool,
    /// Enabled force feedback effects, by [`EFFECTS`] index.
    pub effects: [bool; 11],
}

impl DeviceConfig {
    /// vJoy's defaults for a new device, with all axes like Gremlin profiles expect.
    pub fn new(id: u32) -> Self {
        Self {
            id,
            axes: [true; 8],
            buttons: 32,
            hats: 1,
            discrete_hats: false,
            effects: [false; 11],
        }
    }

    /// Arguments that (re)create this device.
    pub fn create_args(&self) -> Vec<String> {
        let mut args = vec![self.id.to_string(), "-f".into()];
        if self.axes.iter().any(|a| *a) {
            args.push("-a".into());
            args.extend(
                AXES.iter()
                    .zip(self.axes)
                    .filter(|(_, on)| *on)
                    .map(|(n, _)| n.to_string()),
            );
        }
        args.push("-b".into());
        args.push(self.buttons.min(MAX_BUTTONS).to_string());
        args.push(if self.discrete_hats { "-s" } else { "-p" }.into());
        args.push(self.hats.min(MAX_HATS).to_string());
        if self.effects.iter().any(|e| *e) {
            args.push("-e".into());
            args.extend(
                EFFECTS
                    .iter()
                    .zip(self.effects)
                    .filter(|(_, on)| *on)
                    .map(|(n, _)| n.to_string()),
            );
        }
        args
    }

    /// Parses a `vJoyConfig -t -c` line such as `vJoyConfig 1 -f -a X Y Z -b 32 -p 4`.
    pub fn parse(line: &str) -> Option<Self> {
        let mut words = line.split_whitespace();
        if !words.next()?.eq_ignore_ascii_case("vJoyConfig") {
            return None;
        }
        let id: u32 = words.next()?.parse().ok()?;
        let mut c = Self {
            id,
            axes: [false; 8],
            buttons: 0,
            hats: 0,
            discrete_hats: false,
            effects: [false; 11],
        };
        let mut flag = "";
        for w in words {
            if w.starts_with('-') {
                flag = w;
                continue;
            }
            match flag {
                "-a" => {
                    if let Some(i) = AXES.iter().position(|a| a.eq_ignore_ascii_case(w)) {
                        c.axes[i] = true;
                    }
                }
                "-b" => c.buttons = w.parse().unwrap_or(0),
                "-p" => {
                    c.hats = w.parse().unwrap_or(0);
                    c.discrete_hats = false;
                }
                "-s" => {
                    let n = w.parse().unwrap_or(0);
                    if n > 0 {
                        c.hats = n;
                        c.discrete_hats = true;
                    }
                }
                "-e" => {
                    if w.eq_ignore_ascii_case("all") {
                        c.effects = [true; 11];
                    } else if let Some(i) = EFFECTS.iter().position(|e| e.eq_ignore_ascii_case(w)) {
                        c.effects[i] = true;
                    }
                }
                _ => {}
            }
        }
        Some(c)
    }
}

/// Runs a program with administrator rights (UAC prompt) and returns its exit code.
pub fn run_as_admin(tool: &Path, args: &[String]) -> Result<u32, String> {
    #[cfg(windows)]
    {
        use windows::core::{HSTRING, PCWSTR};
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            GetExitCodeProcess, WaitForSingleObject, INFINITE,
        };
        use windows::Win32::UI::Shell::{
            ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
        };
        use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

        let file = HSTRING::from(tool.as_os_str());
        let params = HSTRING::from(
            args.iter()
                .map(|a| {
                    if a.contains(' ') {
                        format!("\"{a}\"")
                    } else {
                        a.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
        );
        let verb = HSTRING::from("runas");
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(params.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        // SAFETY: the strings outlive the call; the process handle is closed below.
        unsafe {
            ShellExecuteExW(&mut info).map_err(|e| e.message())?;
            if info.hProcess.is_invalid() {
                return Err(format!("{} did not start", tool.display()));
            }
            WaitForSingleObject(info.hProcess, INFINITE);
            let mut code = 0u32;
            let _ = GetExitCodeProcess(info.hProcess, &mut code);
            let _ = CloseHandle(info.hProcess);
            Ok(code)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (tool, args);
        Err("vJoy configuration needs Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_writes_config_lines() {
        let c =
            DeviceConfig::parse("vJoyConfig 2 -f -a X Y Z Rx Ry Rz Sl0 Sl1 -b 127 -p 4").unwrap();
        assert_eq!(c.id, 2);
        assert_eq!(c.axes, [true; 8]);
        assert_eq!((c.buttons, c.hats, c.discrete_hats), (127, 4, false));
        assert_eq!(
            c.create_args().join(" "),
            "2 -f -a X Y Z Rx Ry Rz Sl0 Sl1 -b 127 -p 4"
        );

        let d = DeviceConfig::parse("vJoyConfig 5 -f -a x rz -b 8 -s 2 -e Const Fric").unwrap();
        assert_eq!(
            d.axes,
            [true, false, false, false, false, true, false, false]
        );
        assert_eq!((d.hats, d.discrete_hats), (2, true));
        assert!(d.effects[0] && d.effects[10] && !d.effects[1]);
        assert_eq!(
            d.create_args().join(" "),
            "5 -f -a X Rz -b 8 -s 2 -e Const Fric"
        );
        assert!(DeviceConfig::parse("Device 1:\tStatus: FREE").is_none());
    }
}
