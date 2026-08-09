use gtk4::gdk;

pub(crate) fn is_super_key(key: gdk::Key) -> bool {
    key == gdk::Key::Super_L || key == gdk::Key::Super_R
}
