use relm4::adw::{self, prelude::*};
use relm4::gtk;

use crate::system_info::SystemInfo;

pub(crate) fn build_about_page() -> gtk::Widget {
    let info = SystemInfo::gather();

    let page = adw::PreferencesPage::new();
    page.set_title("Sobre");

    let brand = adw::PreferencesGroup::new();
    brand.set_title("TUXI");
    brand.set_description(Some("Ambiente gráfico em desenvolvimento"));

    let logo_row = adw::ActionRow::builder()
        .title("Ambiente")
        .subtitle("TUXI sobre labwc")
        .build();
    let logo = gtk::Image::from_icon_name("distributor-logo");
    logo.set_pixel_size(48);
    logo_row.add_prefix(&logo);
    brand.add(&logo_row);

    let host_row = adw::ActionRow::builder()
        .title("Nome do dispositivo")
        .subtitle(if info.hostname.is_empty() {
            "desconhecido"
        } else {
            &info.hostname
        })
        .build();
    brand.add(&host_row);
    page.add(&brand);

    let os = adw::PreferencesGroup::new();
    os.set_title("Sistema operativo");

    let pretty = adw::ActionRow::builder()
        .title("Distribuição")
        .subtitle(&info.pretty_name)
        .build();
    let distro_icon = gtk::Image::from_icon_name("distributor-logo");
    distro_icon.set_pixel_size(28);
    pretty.add_prefix(&distro_icon);
    os.add(&pretty);

    os.add(
        &adw::ActionRow::builder()
            .title("Nome")
            .subtitle(&info.os_name)
            .build(),
    );
    if !info.os_version.is_empty() {
        os.add(
            &adw::ActionRow::builder()
                .title("Versão")
                .subtitle(&info.os_version)
                .build(),
        );
    }
    os.add(
        &adw::ActionRow::builder()
            .title("Kernel")
            .subtitle(&info.kernel)
            .build(),
    );
    page.add(&os);

    let hw = adw::PreferencesGroup::new();
    hw.set_title("Hardware");
    hw.add(
        &adw::ActionRow::builder()
            .title("Processador")
            .subtitle(&info.cpu_model)
            .build(),
    );
    hw.add(
        &adw::ActionRow::builder()
            .title("Memória")
            .subtitle(info.memory_label())
            .build(),
    );
    page.add(&hw);

    let clamp = adw::Clamp::builder()
        .maximum_size(720)
        .child(&page)
        .build();
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&clamp)
        .build();
    scrolled.add_css_class("tuxi-settings-content");
    scrolled.upcast()
}
