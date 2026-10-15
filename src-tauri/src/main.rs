// Empêche l'ouverture d'une console Windows supplémentaires en release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
	movie_box_lib::run()
}
