use relm4::adw::{self, prelude::*};
use relm4::gtk;

use crate::system_info::SystemInfo;

pub(crate) fn build_about_page() -> gtk::Widget {
    let info = SystemInfo::gather();

    let page = adw::PreferencesPage::new();
    page.set_title("Sobre");

    let brand = adw::PreferencesGroup::new();
    brand.set_title("TUXI");
    brand.set_description(Some("Ambiente gráfico tradicional sobre labwc"));

    let logo_row = adw::ActionRow::builder()
        .title("Ambiente")
        .subtitle(format!("TUXI {}", info.tuxi_version))
        .build();
    let logo = gtk::Image::from_icon_name("applications-system-symbolic");
    logo.set_pixel_size(40);
    logo_row.add_prefix(&logo);
    brand.add(&logo_row);

    brand.add(
        &adw::ActionRow::builder()
            .title("Nome do dispositivo")
            .subtitle(if info.hostname.is_empty() {
                "desconhecido"
            } else {
                &info.hostname
            })
            .build(),
    );
    page.add(&brand);

    let os = adw::PreferencesGroup::new();
    os.set_title("Sistema operativo");

    let pretty = adw::ActionRow::builder()
        .title("Distribuição")
        .subtitle(&info.pretty_name)
        .build();
    let distro_icon = gtk::Image::from_icon_name(info.distro_icon_name());
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

    let stack = adw::PreferencesGroup::new();
    stack.set_title("Ambiente gráfico");
    stack.set_description(Some("Sessão e stack TUXI"));

    stack.add(
        &adw::ActionRow::builder()
            .title("Plataforma gráfica")
            .subtitle(&info.session_type)
            .build(),
    );
    stack.add(
        &adw::ActionRow::builder()
            .title("Compositor")
            .subtitle(&info.compositor)
            .build(),
    );
    stack.add(
        &adw::ActionRow::builder()
            .title("Desktop")
            .subtitle(&info.desktop)
            .build(),
    );
    stack.add(
        &adw::ActionRow::builder()
            .title("GTK")
            .subtitle(&info.gtk_version)
            .build(),
    );
    stack.add(
        &adw::ActionRow::builder()
            .title("Libadwaita / Relm4")
            .subtitle("Libadwaita + Relm4 (Rust)")
            .build(),
    );
    stack.add(
        &adw::ActionRow::builder()
            .title("TUXI (este build)")
            .subtitle(&info.tuxi_version)
            .build(),
    );
    page.add(&stack);

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
