use relm4::adw;
use relm4::gtk::{self, prelude::*};

use crate::page_chrome::{page_header, page_scroller};
use super::PageId;

pub(crate) fn build_stub_page(id: PageId, detail: &str) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.set_hexpand(true);
    root.append(&page_header(id.title(), id.subtitle()));

    let status = adw::StatusPage::builder()
        .icon_name(id.icon_name())
        .title(id.title())
        .description(detail)
        .vexpand(true)
        .hexpand(true)
        .build();
    status.add_css_class("tuxi-settings-stub");
    root.append(&status);

    page_scroller(&root)
}
