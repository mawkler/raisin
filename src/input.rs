use gtk4::gdk;

pub(crate) fn is_super_key(key: gdk::Key) -> bool {
    key == gdk::Key::Super_L || key == gdk::Key::Super_R
}

pub(crate) fn matches_trigger_key(pressed: gdk::Key, trigger_char: char) -> bool {
    let Some(name) = pressed.name() else {
        return false;
    };

    name.as_str().eq_ignore_ascii_case(&trigger_char.to_string())
}
