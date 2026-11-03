//! Páginas da central de configurações.

mod about;
mod displays;
mod stub;

pub(crate) use about::build_about_page;
pub(crate) use displays::build_displays_page;
pub(crate) use stub::build_stub_page;

use relm4::gtk;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PageId {
    Network,
    Bluetooth,
    Displays,
    Power,
    Appearance,
    Region,
    Printers,
    About,
}

impl PageId {
    pub(crate) const ALL: [PageId; 8] = [
        Self::Network,
        Self::Bluetooth,
        Self::Displays,
        Self::Power,
        Self::Appearance,
        Self::Region,
        Self::Printers,
        Self::About,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::Bluetooth => "bluetooth",
            Self::Displays => "displays",
            Self::Power => "power",
            Self::Appearance => "appearance",
            Self::Region => "region",
            Self::Printers => "printers",
            Self::About => "about",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Network => "Rede",
            Self::Bluetooth => "Bluetooth",
            Self::Displays => "Telas",
            Self::Power => "Energia",
            Self::Appearance => "Aparência",
            Self::Region => "Região e idioma",
            Self::Printers => "Impressoras",
            Self::About => "Sobre",
        }
    }

    pub(crate) fn subtitle(self) -> &'static str {
        match self {
            Self::Network => "Wi‑Fi e ligação com fios",
            Self::Bluetooth => "Dispositivos sem fios",
            Self::Displays => "Monitores, resolução e escala",
            Self::Power => "Suspensão e consumo",
            Self::Appearance => "Tema e tipografia",
            Self::Region => "Idioma, fuso e formatos",
            Self::Printers => "Impressão e scanners",
            Self::About => "Sistema e dispositivo",
        }
    }

    pub(crate) fn icon_name(self) -> &'static str {
        match self {
            Self::Network => "network-wireless-symbolic",
            Self::Bluetooth => "bluetooth-active-symbolic",
            Self::Displays => "video-display-symbolic",
            Self::Power => "battery-symbolic",
            Self::Appearance => "preferences-desktop-appearance-symbolic",
            Self::Region => "preferences-desktop-locale-symbolic",
            Self::Printers => "printer-symbolic",
            Self::About => "help-about-symbolic",
        }
    }

    pub(crate) fn matches_query(self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        self.title().to_lowercase().contains(&q)
            || self.subtitle().to_lowercase().contains(&q)
            || self.id().contains(&q)
    }
}

pub(crate) fn build_page(id: PageId) -> gtk::Widget {
    match id {
        PageId::About => build_about_page(),
        PageId::Displays => build_displays_page(),
        PageId::Network => build_stub_page(
            id,
            "Em breve: Wi‑Fi e rede com NetworkManager.",
        ),
        PageId::Bluetooth => build_stub_page(
            id,
            "Em breve: pareamento e dispositivos BlueZ.",
        ),
        PageId::Power => build_stub_page(
            id,
            "Em breve: perfis de energia e suspensão.",
        ),
        PageId::Appearance => build_stub_page(
            id,
            "Em breve: tema claro/escuro e tipografia TUXI.",
        ),
        PageId::Region => build_stub_page(
            id,
            "Em breve: idioma, região e formatos.",
        ),
        PageId::Printers => build_stub_page(
            id,
            "Em breve: impressoras via CUPS.",
        ),
    }
}
