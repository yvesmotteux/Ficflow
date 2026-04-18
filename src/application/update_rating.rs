use crate::domain::fanfiction::{Fanfiction, FanfictionOps, UserRating};
use crate::error::FicflowError;

pub fn update_user_rating(
    fanfiction_ops: &dyn FanfictionOps,
    fic_id: u64,
    rating_str: &str,
) -> Result<Fanfiction, FicflowError> {
    let mut fic = fanfiction_ops.get_fanfiction_by_id(fic_id)?;

    let user_rating = match rating_str.to_lowercase().as_str() {
        "1" | "one" => Some(UserRating::One),
        "2" | "two" => Some(UserRating::Two),
        "3" | "three" => Some(UserRating::Three),
        "4" | "four" => Some(UserRating::Four),
        "5" | "five" => Some(UserRating::Five),
        "0" | "none" | "clear" | "remove" => None,
        _ => {
            return Err(FicflowError::InvalidInput(format!(
                "Invalid rating: '{}'. Valid options are numbers 1-5 or words 'one' through 'five', or 'none' to remove rating",
                rating_str
            )));
        }
    };

    fic.user_rating = user_rating;
    fanfiction_ops.save_fanfiction(&fic)?;

    Ok(fic)
}
