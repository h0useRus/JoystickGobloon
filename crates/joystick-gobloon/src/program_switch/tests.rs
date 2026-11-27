use std::path::PathBuf;

use super::*;

fn app(path: &str, profile: Option<&str>) -> AppEntry {
    let mut a = AppEntry::new(PathBuf::from(path));
    a.profile = profile.map(PathBuf::from);
    a
}

fn program(name: &str) -> Program {
    Program::new(1, PathBuf::from(format!("C:/Other/{name}")))
}

const OWN: &str = "JoystickGobloon.exe";

/// What the app does: apply the due program at once.
trait Check {
    fn check<'a>(
        &mut self,
        front: Option<&Program>,
        apps: &'a [AppEntry],
        own: &str,
    ) -> Option<&'a AppEntry>;
}

impl Check for Switcher {
    fn check<'a>(
        &mut self,
        front: Option<&Program>,
        apps: &'a [AppEntry],
        own: &str,
    ) -> Option<&'a AppEntry> {
        let app = self.due(front, apps, own)?;
        self.applied(app);
        Some(app)
    }
}

fn apps() -> Vec<AppEntry> {
    vec![
        app("C:/Games/One/One.exe", Some("one.xml")),
        app("C:/Games/Two/Two.exe", Some("two.xml")),
        app("C:/Tools/radio.exe", None),
        app("C:/Tools/JoystickGobloon.exe", Some("own.xml")),
    ]
}

#[test]
fn a_program_kept_by_its_windows_path_matches_everywhere() {
    let apps = [app(r"C:\Games\Rally\Rally.EXE", Some("rally.xml"))];
    assert_eq!(apps[0].key(), "rally.exe");
    let front = Program {
        pid: 1,
        name: "Rally.exe".into(),
        path: PathBuf::from(r"C:\Games\Rally\Rally.exe"),
    };
    assert!(choose(Some(&front), &apps, OWN).is_some());
}

#[test]
fn the_program_in_front_chooses_by_file_name() {
    let apps = apps();
    let chosen = choose(Some(&program("ONE.exe")), &apps, OWN).unwrap();
    assert_eq!(chosen.profile, Some(PathBuf::from("one.xml")));
    assert!(
        choose(Some(&program("radio.exe")), &apps, OWN).is_none(),
        "no profile"
    );
    assert!(
        choose(Some(&program("browser.exe")), &apps, OWN).is_none(),
        "not listed"
    );
    assert!(
        choose(Some(&program(OWN)), &apps, OWN).is_none(),
        "never itself"
    );
    assert!(choose(None, &apps, OWN).is_none(), "nothing in front");
}

#[test]
fn any_other_program_gets_the_default_profile() {
    let mut apps = apps();
    let mut other = AppEntry::other();
    other.profile = Some(PathBuf::from("default.xml"));
    apps.push(other);
    let chosen = |name: Option<&str>| {
        let front = name.map(program);
        choose(front.as_ref(), &apps, OWN).and_then(|a| a.profile.clone())
    };
    assert_eq!(chosen(Some("One.exe")), Some(PathBuf::from("one.xml")));
    assert_eq!(
        chosen(Some("browser.exe")),
        Some(PathBuf::from("default.xml"))
    );
    assert_eq!(
        chosen(Some("radio.exe")),
        Some(PathBuf::from("default.xml")),
        "no profile"
    );
    assert_eq!(
        chosen(None),
        Some(PathBuf::from("default.xml")),
        "the desktop"
    );
    assert_eq!(chosen(Some(OWN)), None, "itself keeps the running profile");
    // Back from another program the game's profile runs again, and the other way round.
    let mut switcher = Switcher::default();
    let (one, browser) = (program("One.exe"), program("browser.exe"));
    assert!(switcher.check(Some(&one), &apps, OWN).is_some());
    assert!(switcher.check(Some(&browser), &apps, OWN).is_some());
    assert!(
        switcher
            .check(Some(&program("mail.exe")), &apps, OWN)
            .is_none()
    );
    assert!(switcher.check(Some(&one), &apps, OWN).is_some());
}

#[test]
fn a_profile_is_applied_once_per_program() {
    let apps = apps();
    let mut switcher = Switcher::default();
    let (one, two) = (program("One.exe"), program("Two.exe"));
    assert!(switcher.check(Some(&one), &apps, OWN).is_some());
    // Alt-tab to a browser and back: the running profile stays as it is.
    assert!(
        switcher
            .check(Some(&program("browser.exe")), &apps, OWN)
            .is_none()
    );
    assert!(switcher.check(Some(&one), &apps, OWN).is_none());
    // Another game in front switches; back to the first switches again.
    assert!(switcher.check(Some(&two), &apps, OWN).is_some());
    assert!(switcher.check(Some(&one), &apps, OWN).is_some());
    // Activate applies the program in front even when it chose last time.
    switcher.reset();
    assert!(switcher.check(Some(&one), &apps, OWN).is_some());
}

#[test]
fn due_tells_without_applying() {
    let apps = apps();
    let mut switcher = Switcher::default();
    let one = program("One.exe");
    assert!(switcher.due(Some(&one), &apps, OWN).is_some());
    assert!(switcher.due(Some(&one), &apps, OWN).is_some(), "still due");
    assert!(switcher.check(Some(&one), &apps, OWN).is_some());
    assert!(switcher.due(Some(&one), &apps, OWN).is_none());
}

#[test]
fn a_failed_program_is_tried_again_when_it_comes_back() {
    let apps = apps();
    let mut switcher = Switcher::default();
    let (one, two) = (program("One.exe"), program("Two.exe"));
    switcher.applied(&apps[0]);
    let due = switcher.due(Some(&two), &apps, OWN).unwrap().clone();
    switcher.failed(&due);
    assert!(
        switcher.due(Some(&two), &apps, OWN).is_none(),
        "not every frame"
    );
    switcher.front_changed();
    assert!(
        switcher.due(Some(&one), &apps, OWN).is_none(),
        "One runs already"
    );
    switcher.front_changed();
    assert!(
        switcher.due(Some(&two), &apps, OWN).is_some(),
        "back to Two: again"
    );
}

#[test]
fn controllers_of_the_profile_that_change() {
    let s = |id: &str, by: Option<&str>| Served {
        id: id.into(),
        name: format!("Stick {id}"),
        by: by.map(String::from),
    };
    let before = [
        s("a", Some("a")),
        s("b", None),
        s("c", Some("c")),
        s("d", Some("d")),
    ];
    let after = [
        s("a", None),
        s("b", Some("b")),
        s("c", Some("x")),
        s("d", Some("D")),
        s("e", Some("e")),
    ];
    assert_eq!(
        device_changes(&before, &after),
        [
            DeviceChange::Disconnected("Stick a".into()),
            DeviceChange::Connected("Stick b".into()),
            DeviceChange::Replaced("Stick c".into()),
        ]
    );
}
