use std::rc::Rc;

use relm4::adw;
use relm4::gtk::{self, prelude::*};

use crate::page_chrome::{page_header, page_scroller};
use crate::wlr_randr::{
    apply_output, probe_outputs, ApplyRequest, OutputHead, OutputMode, ProbeResult,
};

pub(crate) fn build_displays_page() -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_hexpand(true);
    root.append(&page_header(
        "Telas",
        "Monitores ligados ao labwc (wlr-output-management).",
    ));

    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    root.append(&content);

    let status = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .css_classes(["tuxi-settings-muted"])
        .build();
    root.append(&status);

    refresh_displays(&content, &status);

    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let refresh_btn = gtk::Button::with_label("Atualizar");
    refresh_btn.add_css_class("tuxi-settings-button");
    let content_r = content.clone();
    let status_r = status.clone();
    refresh_btn.connect_clicked(move |_| {
        refresh_displays(&content_r, &status_r);
    });
    toolbar.append(&refresh_btn);
    root.append(&toolbar);

    page_scroller(&root)
}

fn clear_box(box_: &gtk::Box) {
    while let Some(child) = box_.first_child() {
        box_.remove(&child);
    }
}

fn refresh_displays(content: &gtk::Box, status: &gtk::Label) {
    clear_box(content);
    match probe_outputs() {
        ProbeResult::MissingBinary => {
            status.set_text(
                "wlr-randr não está instalado. No Fedora: sudo dnf install wlr-randr",
            );
            let page = adw::StatusPage::builder()
                .icon_name("video-display-symbolic")
                .title("Sem wlr-randr")
                .description(
                    "A página Telas usa wlr-randr para falar com o labwc.\n\
                     Instale o pacote e clique em Atualizar.",
                )
                .build();
            content.append(&page);
        }
        ProbeResult::Error(err) => {
            status.set_text(&format!("Erro: {err}"));
            let page = adw::StatusPage::builder()
                .icon_name("dialog-warning-symbolic")
                .title("Não foi possível listar monitores")
                .description(err)
                .build();
            content.append(&page);
        }
        ProbeResult::Ok(heads) if heads.is_empty() => {
            status.set_text("Nenhuma saída detectada.");
            content.append(
                &adw::StatusPage::builder()
                    .icon_name("video-display-symbolic")
                    .title("Sem monitores")
                    .description("O compositor não reportou saídas.")
                    .build(),
            );
        }
        ProbeResult::Ok(heads) => {
            status.set_text(&format!(
                "{} monitor(es) · alterações aplicam de imediato no labwc",
                heads.len()
            ));
            for head in heads {
                content.append(&build_output_card(head, content, status));
            }
            if content.observe_children().n_items() >= 2 {
                content.append(&build_layout_hints());
            }
        }
    }
}

fn build_layout_hints() -> gtk::Widget {
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 6);
    box_.add_css_class("tuxi-settings-card");
    box_.append(
        &gtk::Label::builder()
            .label("Arranjo")
            .halign(gtk::Align::Start)
            .css_classes(["heading"])
            .build(),
    );
    box_.append(
        &gtk::Label::builder()
            .label(
                "Com vários monitores, ajuste a posição (X,Y) em cada cartão. \
                 Ex.: lado a lado — secundário em X = largura do principal; \
                 acima — secundário em Y negativo da altura.",
            )
            .halign(gtk::Align::Start)
            .wrap(true)
            .css_classes(["tuxi-settings-muted"])
            .build(),
    );
    box_.upcast()
}

fn build_output_card(
    head: OutputHead,
    content: &gtk::Box,
    status: &gtk::Label,
) -> gtk::Widget {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    card.add_css_class("tuxi-settings-card");

    let title = gtk::Label::builder()
        .label(head.display_title())
        .halign(gtk::Align::Start)
        .wrap(true)
        .css_classes(["heading"])
        .build();
    card.append(&title);

    let enabled = gtk::CheckButton::with_label("Ativado");
    enabled.set_active(head.enabled);
    card.append(&enabled);

    let mode_labels: Vec<String> = head.modes.iter().map(OutputMode::label).collect();
    let mode_model = gtk::StringList::new(&mode_labels.iter().map(String::as_str).collect::<Vec<_>>());
    let mode_drop = gtk::DropDown::new(Some(mode_model), gtk::Expression::NONE);
    if let Some(current) = head.current_mode() {
        if let Some(idx) = head.modes.iter().position(|m| m == current) {
            mode_drop.set_selected(idx as u32);
        }
    }
    card.append(&labeled_row("Resolução", &mode_drop));

    let scales = ["1.00", "1.25", "1.50", "1.75", "2.00"];
    let scale_model = gtk::StringList::new(&scales);
    let scale_drop = gtk::DropDown::new(Some(scale_model), gtk::Expression::NONE);
    let scale_idx = scales
        .iter()
        .position(|s| (s.parse::<f64>().unwrap_or(1.0) - head.scale).abs() < 0.01)
        .unwrap_or(0);
    scale_drop.set_selected(scale_idx as u32);
    card.append(&labeled_row("Escala", &scale_drop));

    let pos_x = gtk::SpinButton::with_range(-8192.0, 8192.0, 1.0);
    pos_x.set_value(f64::from(head.pos_x));
    let pos_y = gtk::SpinButton::with_range(-8192.0, 8192.0, 1.0);
    pos_y.set_value(f64::from(head.pos_y));
    let pos_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    pos_box.append(&gtk::Label::new(Some("X")));
    pos_box.append(&pos_x);
    pos_box.append(&gtk::Label::new(Some("Y")));
    pos_box.append(&pos_y);
    card.append(&labeled_row("Posição", &pos_box));

    let apply = gtk::Button::with_label("Aplicar");
    apply.add_css_class("tuxi-settings-button");
    apply.add_css_class("suggested-action");
    apply.set_halign(gtk::Align::End);

    let modes = Rc::new(head.modes.clone());
    let name = head.name.clone();
    let content_c = content.clone();
    let status_c = status.clone();
    let enabled_c = enabled.clone();
    let mode_drop_c = mode_drop.clone();
    let scale_drop_c = scale_drop.clone();
    let pos_x_c = pos_x.clone();
    let pos_y_c = pos_y.clone();

    apply.connect_clicked(move |_| {
        let selected = mode_drop_c.selected() as usize;
        let mode = modes.get(selected).cloned();
        let scale_s = scales
            .get(scale_drop_c.selected() as usize)
            .copied()
            .unwrap_or("1.00");
        let scale: f64 = scale_s.parse().unwrap_or(1.0);
        let req = ApplyRequest {
            name: name.clone(),
            enabled: enabled_c.is_active(),
            mode,
            pos_x: pos_x_c.value() as i32,
            pos_y: pos_y_c.value() as i32,
            scale,
        };
        match apply_output(&req) {
            Ok(()) => {
                status_c.set_text(&format!("Aplicado em {}", req.name));
                refresh_displays(&content_c, &status_c);
            }
            Err(err) => {
                status_c.set_text(&format!("Falha ao aplicar: {err:#}"));
            }
        }
    });
    card.append(&apply);

    card.upcast()
}

fn labeled_row(label: &str, child: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    row.append(
        &gtk::Label::builder()
            .label(label)
            .halign(gtk::Align::Start)
            .css_classes(["tuxi-settings-muted"])
            .build(),
    );
    row.append(child);
    row
}
