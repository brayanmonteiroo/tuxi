use relm4::adw;
use relm4::gtk::{self, prelude::*};

use super::PageId;

pub(crate) fn build_stub_page(id: PageId, detail: &str) -> gtk::Widget {
    let status = adw::StatusPage::builder()
        .icon_name(id.icon_name())
        .title(id.title())
        .description(format!("{}\n\n{}", id.subtitle(), detail))
        .build();
    status.add_css_class("tuxi-settings-content");
    status.upcast()
}
