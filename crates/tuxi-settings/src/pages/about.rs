use relm4::gtk::{self, prelude::*};

use crate::page_chrome::{info_row, info_row_with_icon, page_header, page_scroller, section};
use crate::system_info::SystemInfo;

pub(crate) fn build_about_page() -> gtk::Widget {
    let info = SystemInfo::gather();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.set_hexpand(true);
    root.append(&page_header(
        "Sobre",
        "Informação do sistema e da sessão TUXI",
    ));

    let (brand, brand_list) = section(
        "TUXI",
        Some("Ambiente gráfico tradicional sobre labwc"),
    );
    brand_list.append(&info_row_with_icon(
        "Ambiente",
        &format!("TUXI {}", info.tuxi_version),
        "applications-system-symbolic",
    ));
    brand_list.append(&info_row(
        "Nome do dispositivo",
        if info.hostname.is_empty() {
            "desconhecido"
        } else {
            &info.hostname
        },
    ));
    root.append(&brand);

    let (os, os_list) = section("Sistema operativo", None);
    os_list.append(&info_row_with_icon(
        "Distribuição",
        &info.pretty_name,
        info.distro_icon_name(),
    ));
    os_list.append(&info_row("Nome", &info.os_name));
    if !info.os_version.is_empty() {
        os_list.append(&info_row("Versão", &info.os_version));
    }
    os_list.append(&info_row("Kernel", &info.kernel));
    root.append(&os);

    let (stack, stack_list) = section("Ambiente gráfico", Some("Sessão e stack TUXI"));
    stack_list.append(&info_row("Plataforma gráfica", &info.session_type));
    stack_list.append(&info_row("Compositor", &info.compositor));
    stack_list.append(&info_row("Desktop", &info.desktop));
    stack_list.append(&info_row("GTK", &info.gtk_version));
    stack_list.append(&info_row(
        "Libadwaita / Relm4",
        "Libadwaita + Relm4 (Rust)",
    ));
    stack_list.append(&info_row("TUXI (este build)", &info.tuxi_version));
    root.append(&stack);

    let (hw, hw_list) = section("Hardware", None);
    hw_list.append(&info_row("Processador", &info.cpu_model));
    hw_list.append(&info_row("Memória", &info.memory_label()));
    root.append(&hw);

    page_scroller(&root)
}
