use steam_shortcuts_util::shortcut::{Shortcut, ShortcutOwned};

pub struct Game {
    pub name: String,
    pub icon: String,
    pub id: String,
}

impl From<Game> for ShortcutOwned {
    fn from(game: Game) -> Self {
        let launch = format!("uplay://launch/{}", game.id);
        Shortcut::new(0, &game.name, &launch, "", &game.icon, "", "").to_owned()
    }
}
