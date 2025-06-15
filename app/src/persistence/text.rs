use fake::Fake;
use models::schemas::typing::TextOptions;

pub fn generate_text(_options: TextOptions) -> String {
    // Generate a random text based on the provided options in the future
    let text = fake::faker::lorem::en::Words(54..64)
        .fake::<Vec<_>>()
        .join(" ");
    return text;
}
