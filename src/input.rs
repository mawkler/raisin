use gtk4::gdk;

pub(crate) fn is_super_key(key: gdk::Key) -> bool {
    key == gdk::Key::Super_L || key == gdk::Key::Super_R
}

pub(crate) fn matches_trigger_key(pressed: gdk::Key, trigger_char: char) -> bool {
    let Some(expected_key) = gdk::Key::from_name(trigger_char.to_string()) else {
        return false;
    };

    let (lower, upper) = expected_key.convert_case();
    pressed == lower || pressed == upper
}
