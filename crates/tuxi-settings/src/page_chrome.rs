//! Shell comum das páginas: largura total, scroll vertical, sem Clamp estilo GNOME.

use relm4::gtk::{self, prelude::*};

/// Envolve o conteúdo da página num scroller a largura total.
pub(crate) fn page_scroller(child: &impl IsA<gtk::Widget>) -> gtk::Widget {
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 0);
    inner.set_hexpand(true);
    inner.set_halign(gtk::Align::Fill);
    inner.set_margin_top(20);
    inner.set_margin_bottom(24);
    inner.set_margin_start(24);
    inner.set_margin_end(24);
    inner.append(child);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .child(&inner)
        .build();
    scrolled.add_css_class("tuxi-settings-content");
    scrolled.upcast()
}

/// Título + subtítulo no topo da página.
pub(crate) fn page_header(title: &str, subtitle: &str) -> gtk::Box {
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 4);
    box_.set_margin_bottom(16);
    box_.append(
        &gtk::Label::builder()
            .label(title)
            .halign(gtk::Align::Start)
            .css_classes(["title-1"])
            .build(),
    );
    box_.append(
        &gtk::Label::builder()
            .label(subtitle)
            .halign(gtk::Align::Start)
            .wrap(true)
            .css_classes(["tuxi-settings-muted"])
            .build(),
    );
    box_
}

/// Secção com título e lista de linhas a largura total.
pub(crate) fn section(title: &str, description: Option<&str>) -> (gtk::Box, gtk::ListBox) {
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 8);
    wrap.set_hexpand(true);
    wrap.set_margin_bottom(18);

    wrap.append(
        &gtk::Label::builder()
            .label(title)
            .halign(gtk::Align::Start)
            .css_classes(["title-3"])
            .build(),
    );
    if let Some(desc) = description {
        wrap.append(
            &gtk::Label::builder()
                .label(desc)
                .halign(gtk::Align::Start)
                .wrap(true)
                .css_classes(["tuxi-settings-muted"])
                .build(),
        );
    }

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.set_hexpand(true);
    list.add_css_class("tuxi-settings-list");
    list.add_css_class("boxed-list");
    wrap.append(&list);
    (wrap, list)
}

/// Linha título | valor que preenche a largura (sem partir letra a letra).
pub(crate) fn info_row(title: &str, value: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);

    let box_ = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    box_.set_hexpand(true);
    box_.set_margin_top(10);
    box_.set_margin_bottom(10);
    box_.set_margin_start(14);
    box_.set_margin_end(14);

    let title_l = gtk::Label::builder()
        .label(title)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .xalign(0.0)
        .width_chars(16)
        .max_width_chars(22)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["tuxi-settings-row-title"])
        .build();
    title_l.set_size_request(140, -1);

    let value_l = gtk::Label::builder()
        .label(value)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .xalign(0.0)
        .hexpand(true)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .selectable(true)
        .css_classes(["tuxi-settings-row-value"])
        .build();

    box_.append(&title_l);
    box_.append(&value_l);
    row.set_child(Some(&box_));
    row
}

pub(crate) fn info_row_with_icon(title: &str, value: &str, icon: &str) -> gtk::ListBoxRow {
    let row = info_row(title, value);
    let Some(box_) = row.child().and_downcast::<gtk::Box>() else {
        return row;
    };
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(22);
    image.set_valign(gtk::Align::Center);
    box_.prepend(&image);
    row
}
