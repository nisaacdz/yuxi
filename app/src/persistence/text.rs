use models::schemas::typing::TextOptions;
use random_word::Lang;

pub fn generate_text(_options: TextOptions) -> String {
    // Generate a random text based on the provided options in the future
    let text = (0..32)
        .map(|_| random_word::get(Lang::En))
        .collect::<Vec<_>>()
        .join(" ");
    text
}
