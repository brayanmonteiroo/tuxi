//! Superfície layer-shell da área de trabalho.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use relm4::adw::prelude::*;
use relm4::gtk::gdk::{self, Key, ModifierType};
use relm4::gtk::gio;
use relm4::gtk::glib;
use relm4::gtk;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use tracing::{info, warn};

use crate::actions;
use crate::layout::{self, DesktopLayout, IconPos};
use crate::scan::{self, DesktopEntry};
use crate::style;
use crate::xdg;

const ICON_W: i32 = 96;
const ICON_H: i32 = 100;
const GRID_PAD: f64 = 16.0;
const GRID_GAP_X: f64 = 12.0;
const GRID_GAP_Y: f64 = 8.0;
const DRAG_THRESHOLD: f64 = 8.0;

fn grid_cell_w() -> f64 {
    f64::from(ICON_W) + GRID_GAP_X
}

fn grid_cell_h() -> f64 {
    f64::from(ICON_H) + GRID_GAP_Y
}

/// Alinha à grelha invisível (estilo KDE — linhas/colunas).
fn snap_to_grid(x: f64, y: f64) -> (f64, f64) {
    let cw = grid_cell_w();
    let ch = grid_cell_h();
    let col = ((x - GRID_PAD) / cw).round().max(0.0);
    let row = ((y - GRID_PAD) / ch).round().max(0.0);
    (GRID_PAD + col * cw, GRID_PAD + row * ch)
}

pub(crate) struct DesktopModel {
    desktop_dir: PathBuf,
    layout: DesktopLayout,
    selected: HashSet<String>,
    fingerprint: String,
    fixed: Option<gtk::Fixed>,
    wallpaper: Option<gtk::Picture>,
    icon_widgets: HashMap<String, gtk::Box>,
}

#[derive(Debug)]
pub(crate) enum DesktopMsg {
    Refresh,
    ClearSelection,
    /// Substitui a seleção (rubber-band / clique vazio).
    SelectSet(HashSet<String>),
    Activate(String),
    SelectToggle(String, bool),
    MoveIcon { name: String, x: f64, y: f64 },
    /// Posição inicial de um ícone acabado de criar (onde foi o clique).
    PlaceNewIcon { name: String, x: f64, y: f64 },
    SaveLayout,
    SetWallpaper(PathBuf),
}

#[relm4::component(pub)]
impl SimpleComponent for DesktopModel {
    type Init = ();
    type Input = DesktopMsg;
    type Output = ();

    view! {
        #[root]
        gtk::Window {
            set_title: Some("TUXI Desktop"),
            set_decorated: false,
            add_css_class: "tuxi-desktop",
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        if let Err(err) = style::load_tuxi_stylesheet() {
            warn!(?err, "CSS desktop");
        }

        let desktop_dir = xdg::desktop_dir();
        if let Err(err) = xdg::ensure_dir(&desktop_dir) {
            warn!(?err, "criar pasta Desktop");
        }
        let _ = xdg::ensure_dir(&xdg::templates_dir());

        // Um único fundo: Picture (wallpaper ou cor CSS). Fixed em overlay a ecrã inteiro.
        let wallpaper = gtk::Picture::new();
        wallpaper.set_content_fit(gtk::ContentFit::Cover);
        wallpaper.set_hexpand(true);
        wallpaper.set_vexpand(true);
        wallpaper.set_halign(gtk::Align::Fill);
        wallpaper.set_valign(gtk::Align::Fill);
        wallpaper.add_css_class("tuxi-desktop-wallpaper");

        let canvas = gtk::Fixed::new();
        canvas.set_hexpand(true);
        canvas.set_vexpand(true);
        canvas.set_halign(gtk::Align::Fill);
        canvas.set_valign(gtk::Align::Fill);
        canvas.add_css_class("tuxi-desktop-canvas");
        // Fixed reporta tamanho natural = bbox dos filhos; forçar expandir no overlay.
        canvas.set_overflow(gtk::Overflow::Visible);

        let overlay = gtk::Overlay::new();
        overlay.set_hexpand(true);
        overlay.set_vexpand(true);
        overlay.set_child(Some(&wallpaper));
        overlay.add_overlay(&canvas);
        // Overlay do Fixed deve receber eventos em toda a área.
        overlay.set_measure_overlay(&canvas, false);
        root.set_child(Some(&overlay));

        let mut model = DesktopModel {
            desktop_dir: desktop_dir.clone(),
            layout: DesktopLayout::load(),
            selected: HashSet::new(),
            fingerprint: String::new(),
            fixed: Some(canvas.clone()),
            wallpaper: Some(wallpaper.clone()),
            icon_widgets: HashMap::new(),
        };

        let widgets = view_output!();

        // Layer-shell depois dos filhos, antes do present — e de novo em realize/map
        // (mesmo padrão do painel; sem isto a superfície fica do tamanho do conteúdo).
        configure_desktop_layer(&root);
        root.connect_realize(|w| {
            configure_desktop_layer(w);
            stretch_desktop_content(w);
            info!(
                is_layer = w.is_layer_window(),
                w = w.width(),
                h = w.height(),
                "desktop realize"
            );
        });
        root.connect_map(|w| {
            configure_desktop_layer(w);
            stretch_desktop_content(w);
            info!(
                is_layer = w.is_layer_window(),
                w = w.width(),
                h = w.height(),
                "desktop map"
            );
        });

        apply_wallpaper(&wallpaper, model.layout.wallpaper.as_ref());

        // Rubber-band + clique no vazio (Target = só o canvas, não os ícones).
        let rubber_active = Rc::new(Cell::new(false));
        install_rubber_band(
            &canvas,
            &overlay,
            sender.clone(),
            rubber_active.clone(),
        );

        let click = gtk::GestureClick::new();
        click.set_button(1);
        click.set_propagation_phase(gtk::PropagationPhase::Target);
        let sender_clear = sender.clone();
        let rubber_click = rubber_active;
        click.connect_released(move |_, _, _, _| {
            request_dismiss_panel_menu();
            if rubber_click.get() {
                return;
            }
            sender_clear.input(DesktopMsg::ClearSelection);
        });
        canvas.add_controller(click);

        let right = gtk::GestureClick::new();
        right.set_button(3);
        right.set_propagation_phase(gtk::PropagationPhase::Target);
        let sender_ctx = sender.clone();
        let canvas_ctx = canvas.clone();
        let root_ctx = root.clone();
        let dir_ctx = desktop_dir.clone();
        right.connect_pressed(move |_, _, x, y| {
            // Como no Plasma: RMB no desktop fecha o menu iniciar.
            request_dismiss_panel_menu();
            show_empty_context_menu(&root_ctx, &canvas_ctx, &dir_ctx, x, y, sender_ctx.clone());
        });
        canvas.add_controller(right);

        let key = gtk::EventControllerKey::new();
        let sender_key = sender.clone();
        key.connect_key_pressed(move |_, key, _, _| {
            if key == Key::Escape {
                sender_key.input(DesktopMsg::ClearSelection);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        root.add_controller(key);

        rebuild_icons(&mut model, &sender);

        let sender_poll = sender.clone();
        glib::timeout_add_local(Duration::from_secs(2), move || {
            sender_poll.input(DesktopMsg::Refresh);
            glib::ControlFlow::Continue
        });

        info!(dir = %desktop_dir.display(), "área de trabalho TUXI");
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            DesktopMsg::Refresh => {
                let fp = scan::fingerprint(&self.desktop_dir);
                if fp != self.fingerprint {
                    rebuild_icons(self, &sender);
                }
            }
            DesktopMsg::ClearSelection => {
                self.selected.clear();
                sync_selection_style(self);
            }
            DesktopMsg::SelectSet(names) => {
                self.selected = names;
                sync_selection_style(self);
            }
            DesktopMsg::Activate(name) => {
                let p = self.desktop_dir.join(&name);
                if let Err(err) = actions::open_path(&p) {
                    warn!(?err, "abrir");
                }
            }
            DesktopMsg::SelectToggle(name, additive) => {
                if !additive {
                    self.selected.clear();
                }
                if additive && self.selected.contains(&name) {
                    self.selected.remove(&name);
                } else {
                    self.selected.insert(name);
                }
                sync_selection_style(self);
            }
            DesktopMsg::MoveIcon { name, x, y } => {
                place_with_swap(self, &name, x, y);
            }
            DesktopMsg::PlaceNewIcon { name, x, y } => {
                place_new_or_push(self, &name, x, y);
                let _ = self.layout.save();
            }
            DesktopMsg::SaveLayout => {
                if let Err(err) = self.layout.save() {
                    warn!(?err, "gravar layout");
                }
            }
            DesktopMsg::SetWallpaper(path) => {
                self.layout.wallpaper = Some(path);
                let _ = self.layout.save();
                if let Some(pic) = &self.wallpaper {
                    apply_wallpaper(pic, self.layout.wallpaper.as_ref());
                }
            }
        }
    }
}

fn configure_desktop_layer(window: &gtk::Window) {
    if !gtk4_layer_shell::is_supported() {
        warn!("layer-shell indisponível");
        window.set_default_size(1280, 720);
        return;
    }

    if !window.is_layer_window() {
        window.init_layer_shell();
    }
    window.set_layer(Layer::Bottom);
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Right, true);
    window.set_anchor(Edge::Top, true);
    window.set_anchor(Edge::Bottom, true);
    window.set_margin(Edge::Left, 0);
    window.set_margin(Edge::Right, 0);
    window.set_margin(Edge::Top, 0);
    window.set_margin(Edge::Bottom, 0);
    // -1: superfície a ecrã inteiro sem zona exclusiva.
    window.set_exclusive_zone(-1);
    window.set_keyboard_mode(KeyboardMode::OnDemand);
    window.set_namespace(Some("tuxi-desktop"));

    if let Some(monitor) = desktop_monitor(window) {
        window.set_monitor(Some(&monitor));
        let g = monitor.geometry();
        info!(
            width = g.width(),
            height = g.height(),
            "desktop layer-shell 4 âncoras (stretch)"
        );
    }
}

/// Picture/Fixed sem conteúdo pedem 0×0; forçar o tamanho do output para o hit-test
/// cobrir o ecrã (senão o RMB cai no menu do labwc).
fn stretch_desktop_content(window: &gtk::Window) {
    let Some(monitor) = desktop_monitor(window) else {
        return;
    };
    let g = monitor.geometry();
    let w = g.width().max(1);
    let h = g.height().max(1);
    if let Some(child) = window.child() {
        child.set_size_request(w, h);
    }
}

fn desktop_monitor(window: &gtk::Window) -> Option<gdk::Monitor> {
    let display = WidgetExt::display(window);
    window
        .surface()
        .and_then(|s| display.monitor_at_surface(&s))
        .or_else(|| display.monitors().item(0).and_downcast::<gdk::Monitor>())
}

fn apply_wallpaper(picture: &gtk::Picture, path: Option<&PathBuf>) {
    if let Some(p) = path.filter(|p| p.is_file()) {
        picture.set_filename(Some(p));
        picture.remove_css_class("tuxi-desktop-wallpaper-empty");
    } else {
        picture.set_paintable(gdk::Paintable::NONE);
        picture.add_css_class("tuxi-desktop-wallpaper-empty");
    }
}

fn rebuild_icons(model: &mut DesktopModel, sender: &ComponentSender<DesktopModel>) {
    let Some(fixed) = model.fixed.clone() else {
        return;
    };
    while let Some(child) = fixed.first_child() {
        fixed.remove(&child);
    }
    model.icon_widgets.clear();

    let entries = match scan::list_desktop_entries(&model.desktop_dir) {
        Ok(e) => e,
        Err(err) => {
            warn!(?err, "scan desktop");
            return;
        }
    };
    model.fingerprint = scan::fingerprint(&model.desktop_dir);

    let mut auto_i = 0usize;
    let mut placed: Vec<(String, f64, f64)> = Vec::new();
    for entry in &entries {
        let (mut x, mut y) = match model.layout.icons.get(&entry.name).copied() {
            Some(pos) if pos.x.is_finite() && pos.y.is_finite() => {
                clamp_icon_pos(&fixed, pos.x, pos.y)
            }
            _ => {
                let pos = auto_position(auto_i);
                auto_i += 1;
                clamp_icon_pos(&fixed, pos.x, pos.y)
            }
        };
        // Nunca duas entradas na mesma célula ao carregar.
        while placed
            .iter()
            .any(|(_, px, py)| same_cell(*px, *py, x, y))
        {
            let pos = auto_position(auto_i);
            auto_i += 1;
            (x, y) = clamp_icon_pos(&fixed, pos.x, pos.y);
        }
        placed.push((entry.name.clone(), x, y));
        model.layout.set_icon(&entry.name, x, y);
    }

    for entry in &entries {
        let (x, y) = model
            .layout
            .icons
            .get(&entry.name)
            .map(|p| (p.x, p.y))
            .unwrap_or((GRID_PAD, GRID_PAD));
        let widget = build_icon_widget(entry, &model.selected, sender);
        fixed.put(&widget, x, y);
        model.icon_widgets.insert(entry.name.clone(), widget);
    }
    let _ = model.layout.save();
}

/// Só limites do ecrã (sem grelha) — usado durante o arrasto para deslizar livre.
fn clamp_bounds(fixed: &gtk::Fixed, x: f64, y: f64) -> (f64, f64) {
    let fw = fixed.width();
    let fh = fixed.height();
    let iw = ICON_W;
    let ih = ICON_H;

    if fw < iw || fh < ih {
        if !(0.0..4000.0).contains(&x) || !(0.0..4000.0).contains(&y) {
            return (GRID_PAD, GRID_PAD);
        }
        return (x.max(0.0), y.max(0.0));
    }

    layout::WorkArea::from_screen(fw, fh).clamp_child(iw, ih, x, y)
}

/// Ao soltar / criar: alinha à grelha e mantém dentro da área.
fn clamp_icon_pos(fixed: &gtk::Fixed, x: f64, y: f64) -> (f64, f64) {
    let (x, y) = snap_to_grid(x, y);
    clamp_bounds(fixed, x, y)
}

fn same_cell(ax: f64, ay: f64, bx: f64, by: f64) -> bool {
    let (ax, ay) = snap_to_grid(ax, ay);
    let (bx, by) = snap_to_grid(bx, by);
    (ax - bx).abs() < 0.5 && (ay - by).abs() < 0.5
}

fn occupant_at(layout: &DesktopLayout, exclude: &str, x: f64, y: f64) -> Option<String> {
    layout.icons.iter().find_map(|(name, pos)| {
        if name == exclude {
            return None;
        }
        if same_cell(pos.x, pos.y, x, y) {
            Some(name.clone())
        } else {
            None
        }
    })
}

fn next_free_cell(
    fixed: &gtk::Fixed,
    layout: &DesktopLayout,
    exclude: &[&str],
) -> (f64, f64) {
    for i in 0..512 {
        let pos = auto_position(i);
        let (x, y) = clamp_icon_pos(fixed, pos.x, pos.y);
        let taken = layout.icons.iter().any(|(name, p)| {
            !exclude.iter().any(|e| *e == name.as_str()) && same_cell(p.x, p.y, x, y)
        });
        if !taken {
            return (x, y);
        }
    }
    (GRID_PAD, GRID_PAD)
}

fn move_widget(model: &DesktopModel, name: &str, x: f64, y: f64) {
    if let Some(fixed) = &model.fixed {
        if let Some(widget) = model.icon_widgets.get(name) {
            fixed.move_(widget, x, y);
        }
    }
}

/// Soltar ícone: se a célula está ocupada, troca de lugar com o ocupante.
fn place_with_swap(model: &mut DesktopModel, name: &str, x: f64, y: f64) {
    let Some(fixed) = model.fixed.clone() else {
        return;
    };
    let (tx, ty) = clamp_icon_pos(&fixed, x, y);
    let from = model
        .layout
        .icons
        .get(name)
        .copied()
        .unwrap_or(IconPos { x: tx, y: ty });

    if let Some(other) = occupant_at(&model.layout, name, tx, ty) {
        let (ox, oy) = clamp_icon_pos(&fixed, from.x, from.y);
        let (dx, dy) = if same_cell(ox, oy, tx, ty) {
            next_free_cell(&fixed, &model.layout, &[name, other.as_str()])
        } else if occupant_at(&model.layout, &other, ox, oy)
            .is_some_and(|n| n != name)
        {
            next_free_cell(&fixed, &model.layout, &[name, other.as_str()])
        } else {
            (ox, oy)
        };
        model.layout.set_icon(&other, dx, dy);
        move_widget(model, &other, dx, dy);
    }

    model.layout.set_icon(name, tx, ty);
    move_widget(model, name, tx, ty);
}

/// Novo ícone no clique: ocupa a célula; o que estava lá é empurrado para a
/// próxima livre (substituição).
fn place_new_or_push(model: &mut DesktopModel, name: &str, x: f64, y: f64) {
    let (tx, ty) = if let Some(fixed) = model.fixed.clone() {
        let (tx, ty) = clamp_icon_pos(&fixed, x, y);
        if let Some(other) = occupant_at(&model.layout, name, tx, ty) {
            let (fx, fy) = next_free_cell(&fixed, &model.layout, &[name, other.as_str()]);
            model.layout.set_icon(&other, fx, fy);
            move_widget(model, &other, fx, fy);
        }
        (tx, ty)
    } else {
        snap_to_grid(x.max(0.0), y.max(0.0))
    };
    model.layout.set_icon(name, tx, ty);
}

fn auto_position(index: usize) -> IconPos {
    let col = index % 8;
    let row = index / 8;
    IconPos {
        x: GRID_PAD + col as f64 * grid_cell_w(),
        y: GRID_PAD + row as f64 * grid_cell_h(),
    }
}

fn sync_selection_style(model: &DesktopModel) {
    for (name, widget) in &model.icon_widgets {
        if model.selected.contains(name) {
            widget.add_css_class("selected");
        } else {
            widget.remove_css_class("selected");
        }
    }
}

fn build_icon_widget(
    entry: &DesktopEntry,
    selected: &HashSet<String>,
    sender: &ComponentSender<DesktopModel>,
) -> gtk::Box {
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 4);
    box_.set_size_request(ICON_W, ICON_H);
    box_.set_widget_name(&entry.name);
    box_.add_css_class("tuxi-desktop-icon");
    if selected.contains(&entry.name) {
        box_.add_css_class("selected");
    }

    let image = gtk::Image::from_icon_name(if entry.is_dir {
        "folder"
    } else {
        "text-x-generic"
    });
    image.set_pixel_size(48);
    if let Ok(info) = gio::File::for_path(&entry.path).query_info(
        "standard::icon",
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    ) {
        if let Some(gicon) = info.icon() {
            image.set_from_gicon(&gicon);
        }
    }

    let label = gtk::Label::builder()
        .label(&entry.name)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .lines(2)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .justify(gtk::Justification::Center)
        .max_width_chars(12)
        .build();

    box_.append(&image);
    box_.append(&label);

    // Posição absoluta no Fixed (grab offset), clampada à área — evita escala
    // errada de deltas de superfície e ícones a "fugirem" do ecrã.
    let dragged = Rc::new(Cell::new(false));
    let grab = Rc::new(Cell::new(None::<(f64, f64)>)); // offset rato→canto do ícone
    let begin_ptr = Rc::new(Cell::new(None::<(f64, f64)>));

    let drag = gtk::GestureDrag::new();
    drag.set_button(1);
    let dragged_b = dragged.clone();
    let grab_b = grab.clone();
    let begin_b = begin_ptr.clone();
    drag.connect_drag_begin(move |gesture, _, _| {
        dragged_b.set(false);
        grab_b.set(None);
        begin_b.set(None);
        let Some(widget) = gesture.widget() else {
            return;
        };
        let Some(fixed) = widget.parent().and_downcast::<gtk::Fixed>() else {
            return;
        };
        let Some((px, py)) = pointer_in_fixed(gesture, &fixed) else {
            return;
        };
        let (ix, iy) = fixed.child_position(&widget);
        grab_b.set(Some((px - ix, py - iy)));
        begin_b.set(Some((px, py)));
    });

    let dragged_u = dragged.clone();
    let grab_u = grab.clone();
    let begin_u = begin_ptr.clone();
    drag.connect_drag_update(move |gesture, _, _| {
        let Some(widget) = gesture.widget() else {
            return;
        };
        let Some(fixed) = widget.parent().and_downcast::<gtk::Fixed>() else {
            return;
        };
        let Some((px, py)) = pointer_in_fixed(gesture, &fixed) else {
            return;
        };
        let Some((gx, gy)) = grab_u.get() else {
            return;
        };

        if !dragged_u.get() {
            if let Some((bx, by)) = begin_u.get() {
                if (px - bx).hypot(py - by) < DRAG_THRESHOLD {
                    return;
                }
            }
            dragged_u.set(true);
        }

        // Arrasto livre (estilo KDE); a grelha só aplica ao soltar.
        let (nx, ny) = clamp_bounds(&fixed, px - gx, py - gy);
        fixed.move_(&widget, nx, ny);
    });

    let sender_e = sender.clone();
    let name_e = entry.name.clone();
    let dragged_e = dragged.clone();
    let grab_e = grab;
    let begin_e = begin_ptr;
    drag.connect_drag_end(move |gesture, _, _| {
        grab_e.set(None);
        begin_e.set(None);
        if !dragged_e.get() {
            return;
        }
        if let Some(widget) = gesture.widget() {
            if let Some(fixed) = widget.parent().and_downcast::<gtk::Fixed>() {
                let (x, y) = fixed.child_position(&widget);
                let (x, y) = clamp_icon_pos(&fixed, x, y);
                fixed.move_(&widget, x, y);
                sender_e.input(DesktopMsg::MoveIcon {
                    name: name_e.clone(),
                    x,
                    y,
                });
            }
        }
        sender_e.input(DesktopMsg::SaveLayout);
    });
    box_.add_controller(drag);

    let name = entry.name.clone();
    let sender_act = sender.clone();
    let dragged_c = dragged;
    let click = gtk::GestureClick::new();
    click.set_button(1);
    click.connect_released(move |gesture, n_press, _, _| {
        if dragged_c.get() {
            return;
        }
        let additive = gesture
            .current_event_state()
            .contains(ModifierType::CONTROL_MASK);
        if n_press >= 2 {
            sender_act.input(DesktopMsg::Activate(name.clone()));
        } else {
            sender_act.input(DesktopMsg::SelectToggle(name.clone(), additive));
        }
    });
    box_.add_controller(click);

    box_
}

/// Ponteiro em coordenadas do Fixed (não do ícone), para o arrasto não depender
/// do widget a mover-se sob o gesto.
fn pointer_in_fixed(gesture: &gtk::GestureDrag, fixed: &gtk::Fixed) -> Option<(f64, f64)> {
    let event = gesture.current_event()?;
    let (sx, sy) = event.position()?;
    let native = fixed.native()?;
    let (tx, ty) = native.surface_transform();
    let native_w: gtk::Widget = native.upcast();
    let pt = native_w.compute_point(
        fixed,
        &gtk::graphene::Point::new((sx - tx) as f32, (sy - ty) as f32),
    )?;
    Some((f64::from(pt.x()), f64::from(pt.y())))
}

fn event_surface_pos(gesture: &gtk::GestureDrag) -> Option<(f64, f64)> {
    gesture.current_event()?.position()
}

fn show_empty_context_menu(
    root: &gtk::Window,
    canvas: &gtk::Fixed,
    desktop_dir: &PathBuf,
    x: f64,
    y: f64,
    sender: ComponentSender<DesktopModel>,
) {
    let pop = gtk::Popover::new();
    pop.set_parent(canvas);
    pop.set_has_arrow(false);
    pop.set_autohide(true);
    pop.set_cascade_popdown(true);
    pop.add_css_class("tuxi-desktop-menu");

    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 0);
    box_.add_css_class("tuxi-desktop-menu-box");

    // Submenu "Criar novo" (estilo KDE).
    append_create_submenu(
        &box_,
        &pop,
        root,
        desktop_dir,
        x,
        y,
        sender.clone(),
    );

    box_.append(&menu_separator());

    append_menu_row(
        &box_,
        "preferences-desktop-display-symbolic",
        "Configurações de tela",
        {
            let p = pop.clone();
            move || {
                p.popdown();
                if let Err(err) = actions::open_display_settings() {
                    warn!(?err, "telas");
                }
            }
        },
    );

    append_menu_row(
        &box_,
        "preferences-desktop-wallpaper-symbolic",
        "Alterar papel de parede…",
        {
            let p = pop.clone();
            let r = root.clone();
            let s = sender.clone();
            move || {
                p.popdown();
                pick_wallpaper(&r, s.clone());
            }
        },
    );

    box_.append(&menu_separator());

    append_menu_row(&box_, "utilities-terminal-symbolic", "Abrir terminal", {
        let dir = desktop_dir.clone();
        let p = pop.clone();
        move || {
            p.popdown();
            if let Err(err) = actions::open_terminal_at(&dir) {
                warn!(?err, "terminal");
            }
        }
    });

    append_menu_row(&box_, "system-lock-screen-symbolic", "Bloquear tela", {
        let p = pop.clone();
        move || {
            p.popdown();
            if let Err(err) = actions::lock_session() {
                warn!(?err, "lock");
            }
        }
    });

    pop.set_child(Some(&box_));
    place_context_popover(&pop, canvas, x, y);
    pop.popup();
}

/// Posicionamento estilo KDE: canto do menu no rato; vira se não couber.
fn place_context_popover(pop: &gtk::Popover, canvas: &gtk::Fixed, x: f64, y: f64) {
    pop.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));

    let cw = f64::from(canvas.width()).max(1.0);
    let ch = f64::from(canvas.height()).max(1.0);
    // Estimativa até mapear; no map afinamos o offset.
    let est_w = 240.0;
    let est_h = 200.0;

    let open_left = x + est_w > cw;
    let open_up = y + est_h > ch - f64::from(layout::PANEL_RESERVE_BOTTOM);

    let position = match (open_up, open_left) {
        (false, false) => gtk::PositionType::Bottom, // abre para baixo-direita
        (false, true) => gtk::PositionType::Left,    // abre para a esquerda
        (true, false) => gtk::PositionType::Top,     // abre para cima
        (true, true) => gtk::PositionType::Top,      // canto inferior-direito
    };
    pop.set_position(position);

    let pop_c = pop.clone();
    let pos = position;
    pop.connect_map(move |p| {
        // GTK centra o popover no ponto; deslocar para o canto ficar no rato.
        let w = p.width().max(1);
        let h = p.height().max(1);
        let (ox, oy) = match pos {
            gtk::PositionType::Bottom => (w / 2, 0), // centro→canto esquerdo
            gtk::PositionType::Top => (w / 2, 0),
            gtk::PositionType::Left => (0, h / 2), // centro→canto superior
            gtk::PositionType::Right => (0, h / 2),
            _ => (w / 2, 0),
        };
        pop_c.set_offset(ox, oy);
    });
}

fn menu_separator() -> gtk::Separator {
    let s = gtk::Separator::new(gtk::Orientation::Horizontal);
    s.add_css_class("tuxi-desktop-menu-sep");
    s
}

fn append_menu_row(
    parent: &gtk::Box,
    icon: &str,
    label: &str,
    on_click: impl Fn() + 'static,
) {
    let btn = gtk::Button::new();
    btn.set_halign(gtk::Align::Fill);
    btn.add_css_class("flat");
    btn.add_css_class("tuxi-desktop-menu-item");

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_halign(gtk::Align::Fill);

    let img = gtk::Image::from_icon_name(icon);
    img.set_pixel_size(16);
    img.set_halign(gtk::Align::Start);
    img.set_valign(gtk::Align::Center);

    let lbl = gtk::Label::new(Some(label));
    lbl.set_halign(gtk::Align::Start);
    lbl.set_hexpand(true);
    lbl.set_xalign(0.0);

    row.append(&img);
    row.append(&lbl);
    btn.set_child(Some(&row));
    btn.connect_clicked(move |_| on_click());
    parent.append(&btn);
}

fn append_create_submenu(
    parent: &gtk::Box,
    main_pop: &gtk::Popover,
    root: &gtk::Window,
    desktop_dir: &PathBuf,
    click_x: f64,
    click_y: f64,
    sender: ComponentSender<DesktopModel>,
) {
    let btn = gtk::Button::new();
    btn.set_halign(gtk::Align::Fill);
    btn.add_css_class("flat");
    btn.add_css_class("tuxi-desktop-menu-item");

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let img = gtk::Image::from_icon_name("document-new-symbolic");
    img.set_pixel_size(16);
    let lbl = gtk::Label::new(Some("Criar novo"));
    lbl.set_halign(gtk::Align::Start);
    lbl.set_hexpand(true);
    lbl.set_xalign(0.0);
    let chevron = gtk::Image::from_icon_name("go-next-symbolic");
    chevron.set_pixel_size(12);
    chevron.add_css_class("tuxi-desktop-menu-chevron");
    row.append(&img);
    row.append(&lbl);
    row.append(&chevron);
    btn.set_child(Some(&row));

    let sub = gtk::Popover::new();
    sub.set_parent(&btn);
    sub.set_has_arrow(false);
    sub.set_autohide(true);
    sub.set_position(gtk::PositionType::Right);
    sub.add_css_class("tuxi-desktop-menu");

    let sub_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sub_box.add_css_class("tuxi-desktop-menu-box");

    let close_menus = Rc::new({
        let main_pop = main_pop.clone();
        let sub = sub.clone();
        move || {
            sub.popdown();
            main_pop.popdown();
        }
    });

    append_menu_row(&sub_box, "folder-new-symbolic", "Pasta", {
        let dir = desktop_dir.clone();
        let s = sender.clone();
        let r = root.clone();
        let close = close_menus.clone();
        move || {
            close();
            present_name_dialog(
                &r,
                "Nova pasta",
                &actions::suggested_unique_name(&dir, "Nova pasta"),
                {
                    let dir = dir.clone();
                    let s = s.clone();
                    move |name| match actions::create_folder(&dir, &name) {
                        Ok(path) => place_created(&path, click_x, click_y, &s),
                        Err(err) => warn!(?err, "criar pasta"),
                    }
                },
            );
        }
    });

    if let Some(text_tpl) = find_template(&["Texto.txt", "Arquivo de texto.txt"]) {
        let default_name =
            actions::suggested_unique_name(desktop_dir, "Arquivo de texto.txt");
        append_menu_row(
            &sub_box,
            "text-x-generic-symbolic",
            "Arquivo de texto…",
            {
                let dir = desktop_dir.clone();
                let s = sender.clone();
                let r = root.clone();
                let close = close_menus.clone();
                let t = text_tpl;
                move || {
                    close();
                    present_name_dialog(&r, "Arquivo de texto", &default_name, {
                        let dir = dir.clone();
                        let s = s.clone();
                        let t = t.clone();
                        move |name| match actions::create_from_template(&dir, &t, &name) {
                            Ok(path) => place_created(&path, click_x, click_y, &s),
                            Err(err) => warn!(?err, "criar texto"),
                        }
                    });
                }
            },
        );
    }

    append_menu_row(&sub_box, "text-x-generic-symbolic", "Arquivo vazio…", {
        let dir = desktop_dir.clone();
        let s = sender.clone();
        let r = root.clone();
        let close = close_menus.clone();
        move || {
            close();
            present_name_dialog(
                &r,
                "Arquivo vazio",
                &actions::suggested_unique_name(&dir, "Arquivo vazio"),
                {
                    let dir = dir.clone();
                    let s = s.clone();
                    move |name| match actions::create_empty_file(&dir, &name) {
                        Ok(path) => place_created(&path, click_x, click_y, &s),
                        Err(err) => warn!(?err, "criar ficheiro"),
                    }
                },
            );
        }
    });

    if let Some(html_tpl) = find_template(&["HTML.html", "Documento HTML.html"]) {
        let default_name = actions::suggested_unique_name(desktop_dir, "Arquivo HTML.html");
        append_menu_row(&sub_box, "text-html-symbolic", "Arquivo HTML…", {
            let dir = desktop_dir.clone();
            let s = sender.clone();
            let r = root.clone();
            let close = close_menus.clone();
            let t = html_tpl;
            move || {
                close();
                present_name_dialog(&r, "Arquivo HTML", &default_name, {
                    let dir = dir.clone();
                    let s = s.clone();
                    let t = t.clone();
                    move |name| match actions::create_from_template(&dir, &t, &name) {
                        Ok(path) => place_created(&path, click_x, click_y, &s),
                        Err(err) => warn!(?err, "criar HTML"),
                    }
                });
            }
        });
    }

    sub.set_child(Some(&sub_box));

    let root_c = root.clone();
    let sub_c = sub;
    btn.connect_clicked(move |b| {
        let win_w = f64::from(root_c.width()).max(1.0);
        if let Some(pt) = b.compute_point(&root_c, &gtk::graphene::Point::zero()) {
            if f64::from(pt.x()) + 280.0 > win_w {
                sub_c.set_position(gtk::PositionType::Left);
            } else {
                sub_c.set_position(gtk::PositionType::Right);
            }
        }
        sub_c.popup();
    });

    parent.append(&btn);
}

fn place_created(
    path: &std::path::Path,
    click_x: f64,
    click_y: f64,
    sender: &ComponentSender<DesktopModel>,
) {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        sender.input(DesktopMsg::Refresh);
        return;
    };
    sender.input(DesktopMsg::PlaceNewIcon {
        name: name.to_string(),
        x: click_x,
        y: click_y,
    });
    sender.input(DesktopMsg::Refresh);
}

fn find_template(names: &[&str]) -> Option<PathBuf> {
    let files = xdg::list_template_files();
    for want in names {
        if let Some(p) = files.iter().find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(want))
        }) {
            return Some(p.clone());
        }
    }
    None
}

fn request_dismiss_panel_menu() {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let path = runtime.join("tuxi-dismiss-menu");
    let _ = fs::write(&path, b"1");
}

fn install_rubber_band(
    canvas: &gtk::Fixed,
    overlay: &gtk::Overlay,
    sender: ComponentSender<DesktopModel>,
    rubber_active: Rc<Cell<bool>>,
) {
    let band = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    band.add_css_class("tuxi-desktop-rubberband");
    band.set_halign(gtk::Align::Start);
    band.set_valign(gtk::Align::Start);
    band.set_visible(false);
    band.set_can_target(false);
    overlay.add_overlay(&band);

    let origin = Rc::new(Cell::new((0.0_f64, 0.0_f64)));
    let dragging = Rc::new(Cell::new(false));

    let drag = gtk::GestureDrag::new();
    drag.set_button(1);
    drag.set_propagation_phase(gtk::PropagationPhase::Target);

    let origin_b = origin.clone();
    let dragging_b = dragging.clone();
    let active_b = rubber_active.clone();
    let band_b = band.clone();
    drag.connect_drag_begin(move |gesture, x, y| {
        // Coordenadas do gesto são relativas ao widget alvo (canvas).
        let _ = gesture;
        origin_b.set((x, y));
        dragging_b.set(false);
        active_b.set(false);
        band_b.set_visible(false);
    });

    let origin_u = origin.clone();
    let dragging_u = dragging.clone();
    let active_u = rubber_active.clone();
    let band_u = band.clone();
    let canvas_u = canvas.clone();
    let sender_u = sender.clone();
    drag.connect_drag_update(move |_, offset_x, offset_y| {
        let (ox, oy) = origin_u.get();
        let dist = offset_x.hypot(offset_y);
        if !dragging_u.get() {
            if dist < DRAG_THRESHOLD {
                return;
            }
            dragging_u.set(true);
            active_u.set(true);
            request_dismiss_panel_menu();
            band_u.set_visible(true);
        }

        let x1 = ox.min(ox + offset_x);
        let y1 = oy.min(oy + offset_y);
        let x2 = ox.max(ox + offset_x);
        let y2 = oy.max(oy + offset_y);
        let w = (x2 - x1).max(1.0);
        let h = (y2 - y1).max(1.0);

        band_u.set_margin_start(x1 as i32);
        band_u.set_margin_top(y1 as i32);
        band_u.set_size_request(w as i32, h as i32);

        let mut hit = HashSet::new();
        let mut child = canvas_u.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            let Ok(icon) = widget.downcast::<gtk::Box>() else {
                continue;
            };
            if !icon.has_css_class("tuxi-desktop-icon") {
                continue;
            }
            let (ix, iy) = canvas_u.child_position(&icon);
            let iw = f64::from(icon.width().max(ICON_W));
            let ih = f64::from(icon.height().max(ICON_H));
            let intersects = !(ix + iw < x1 || ix > x2 || iy + ih < y1 || iy > y2);
            if intersects {
                // Nome no tooltip ou no accessible? Guardamos no widget_name.
                let name = icon.widget_name().to_string();
                if !name.is_empty() {
                    hit.insert(name);
                }
            }
        }
        sender_u.input(DesktopMsg::SelectSet(hit));
    });

    let dragging_e = dragging;
    let active_e = rubber_active;
    let band_e = band;
    drag.connect_drag_end(move |_, _, _| {
        band_e.set_visible(false);
        band_e.set_size_request(1, 1);
        // Mantém active até ao released do GestureClick limpar o flag.
        if !dragging_e.get() {
            active_e.set(false);
        } else {
            // Próximo idle: permite ClearSelection no próximo clique limpo.
            let active = active_e.clone();
            glib::idle_add_local_once(move || {
                active.set(false);
            });
        }
        dragging_e.set(false);
    });

    canvas.add_controller(drag);
}

/// Modal centrada (arrastável pela barra), chrome TUXI + bloqueia cliques atrás.
fn present_name_dialog(
    parent: &gtk::Window,
    title: &str,
    initial: &str,
    on_create: impl Fn(String) + 'static,
) {
    let Some(overlay) = parent.child().and_downcast::<gtk::Overlay>() else {
        warn!("desktop sem overlay — modal indisponível");
        return;
    };

    let dim = gtk::Box::new(gtk::Orientation::Vertical, 0);
    dim.set_hexpand(true);
    dim.set_vexpand(true);
    dim.set_halign(gtk::Align::Fill);
    dim.set_valign(gtk::Align::Fill);
    dim.add_css_class("tuxi-desktop-modal-dim");
    dim.set_can_target(true);

    for button in [1u32, 2, 3] {
        let sink = gtk::GestureClick::new();
        sink.set_button(button);
        sink.connect_pressed(|_, _, _, _| {});
        dim.add_controller(sink);
    }

    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("tuxi-desktop-modal-card");
    card.set_size_request(400, -1);
    card.set_hexpand(false);
    card.set_vexpand(false);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.add_css_class("tuxi-desktop-modal-header");
    header.set_cursor_from_name(Some("grab"));

    let title_l = gtk::Label::new(Some(title));
    title_l.set_halign(gtk::Align::Start);
    title_l.set_hexpand(true);
    title_l.add_css_class("tuxi-desktop-modal-title");

    let close = gtk::Button::from_icon_name("window-close-symbolic");
    close.set_tooltip_text(Some("Fechar"));
    close.set_focus_on_click(false);
    close.add_css_class("tuxi-winbtn");
    close.add_css_class("tuxi-winbtn-close");

    header.append(&title_l);
    header.append(&close);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.add_css_class("tuxi-desktop-modal-body");

    let hint = gtk::Label::new(Some("Escolhe um nome:"));
    hint.set_halign(gtk::Align::Start);
    hint.add_css_class("dim-label");

    let entry = gtk::Entry::builder()
        .text(initial)
        .activates_default(true)
        .hexpand(true)
        .build();

    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancelar");
    cancel.add_css_class("tuxi-desktop-modal-btn");
    let create = gtk::Button::with_label("Criar");
    create.add_css_class("tuxi-desktop-modal-btn");
    create.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&create);

    body.append(&hint);
    body.append(&entry);
    body.append(&buttons);

    card.append(&header);
    card.append(&body);

    // Fixed + WorkArea: centro real (GTK Align falhava em Box/Overlay).
    let stage = gtk::Fixed::new();
    stage.set_hexpand(true);
    stage.set_vexpand(true);
    stage.set_halign(gtk::Align::Fill);
    stage.set_valign(gtk::Align::Fill);
    stage.set_margin_bottom(layout::PANEL_RESERVE_BOTTOM);
    dim.append(&stage);
    overlay.add_overlay(&dim);

    fn work_area_of(parent: &gtk::Window, stage: &gtk::Fixed) -> layout::WorkArea {
        let mut w = stage.width();
        let mut h = stage.height();
        if w < 64 || h < 64 {
            if let Some(mon) = desktop_monitor(parent) {
                let g = mon.geometry();
                w = g.width();
                h = (g.height() - layout::PANEL_RESERVE_BOTTOM).max(1);
            }
        }
        layout::WorkArea {
            width: w.max(1),
            height: h.max(1),
        }
    }

    fn place_modal_card(parent: &gtk::Window, stage: &gtk::Fixed, card: &gtk::Box) {
        let cw = card.width().max(card.size_request().0.max(400));
        let ch = card.height().max(card.size_request().1.max(120));
        let wa = work_area_of(parent, stage);
        let (x, y) = wa.center_child(cw, ch);
        if card.parent().as_ref() == Some(stage.upcast_ref()) {
            stage.move_(card, x, y);
        } else {
            stage.put(card, x, y);
        }
    }

    stage.put(&card, 0.0, 0.0);
    let placed = Rc::new(Cell::new(false));
    // Reposiciona até ter tamanho real (GTK4 não expõe size-allocate).
    {
        let parent_p = parent.clone();
        let stage_p = stage.clone();
        let card_p = card.clone();
        let placed_p = placed.clone();
        let attempts = Rc::new(Cell::new(0_u32));
        let attempts_c = attempts.clone();
        glib::timeout_add_local(Duration::from_millis(16), move || {
            if placed_p.get() {
                return glib::ControlFlow::Break;
            }
            let n = attempts_c.get();
            if n > 30 {
                return glib::ControlFlow::Break;
            }
            attempts_c.set(n + 1);
            if card_p.width() > 1 && card_p.height() > 1 {
                place_modal_card(&parent_p, &stage_p, &card_p);
                placed_p.set(true);
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }

    let last_surface = Rc::new(Cell::new(None::<(f64, f64)>));
    let drag = gtk::GestureDrag::new();
    drag.set_button(1);
    let last_b = last_surface.clone();
    let card_b = card.clone();
    let stage_b = stage.clone();
    let parent_b = parent.clone();
    let placed_b = placed;
    drag.connect_drag_begin(move |gesture, _, _| {
        last_b.set(event_surface_pos(gesture));
        placed_b.set(true);
        let cw = card_b.width().max(1);
        let ch = card_b.height().max(1);
        let wa = work_area_of(&parent_b, &stage_b);
        let (cx, cy) = stage_b.child_position(&card_b);
        let (x, y) = wa.clamp_child(cw, ch, cx, cy);
        stage_b.move_(&card_b, x, y);
    });
    let last_u = last_surface;
    let card_u = card.clone();
    let stage_u = stage.clone();
    let parent_u = parent.clone();
    drag.connect_drag_update(move |gesture, _, _| {
        let Some(now) = event_surface_pos(gesture) else {
            return;
        };
        let Some(prev) = last_u.get() else {
            last_u.set(Some(now));
            return;
        };
        last_u.set(Some(now));
        let (ddx, ddy) = (now.0 - prev.0, now.1 - prev.1);
        if ddx == 0.0 && ddy == 0.0 {
            return;
        }
        let cw = card_u.width().max(1);
        let ch = card_u.height().max(1);
        let wa = work_area_of(&parent_u, &stage_u);
        let (cx, cy) = stage_u.child_position(&card_u);
        let (nx, ny) = wa.clamp_child(cw, ch, cx + ddx, cy + ddy);
        stage_u.move_(&card_u, nx, ny);
    });
    header.add_controller(drag);

    let dismiss = Rc::new({
        let overlay = overlay.clone();
        let dim = dim.clone();
        move || {
            overlay.remove_overlay(&dim);
        }
    });

    close.connect_clicked({
        let dismiss = dismiss.clone();
        move |_| dismiss()
    });
    cancel.connect_clicked({
        let dismiss = dismiss.clone();
        move |_| dismiss()
    });

    let do_create = Rc::new({
        let entry = entry.clone();
        let dismiss = dismiss.clone();
        move || {
            let name = entry.text().to_string();
            dismiss();
            on_create(name);
        }
    });

    create.connect_clicked({
        let do_create = do_create.clone();
        move |_| do_create()
    });
    entry.connect_activate({
        let do_create = do_create.clone();
        move |_| do_create()
    });

    let key = gtk::EventControllerKey::new();
    key.connect_key_pressed({
        let dismiss = dismiss.clone();
        move |_, key, _, _| {
            if key == Key::Escape {
                dismiss();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        }
    });
    dim.add_controller(key);

    entry.grab_focus();
    entry.select_region(0, -1);
}

fn pick_wallpaper(root: &gtk::Window, sender: ComponentSender<DesktopModel>) {
    let dialog = gtk::FileDialog::builder()
        .title("Escolher papel de parede")
        .modal(true)
        .build();
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    let images = gtk::FileFilter::new();
    images.set_name(Some("Imagens"));
    images.add_pixbuf_formats();
    filters.append(&images);
    dialog.set_filters(Some(&filters));

    let root = root.clone();
    dialog.open(Some(&root), gio::Cancellable::NONE, move |res| {
        if let Ok(file) = res {
            if let Some(path) = file.path() {
                sender.input(DesktopMsg::SetWallpaper(path));
            }
        }
    });
}
