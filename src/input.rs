use gtk4::gdk;

pub(crate) fn is_super_key(key: gdk::Key) -> bool {
    key == gdk::Key::Super_L || key == gdk::Key::Super_R
}

pub(crate) fn matches_trigger_key(pressed: gdk::Key, trigger_char: char) -> bool {
    let Some(name) = pressed.name() else {
        return false;
    };

    let lower = trigger_char.to_lowercase().to_string();
    let upper = trigger_char.to_uppercase().to_string();
    name.as_str() == lower || name.as_str() == upper
}
