use crate::domain::fanfiction::{Fanfiction, FanfictionOps, UserRating};
use crate::error::FicflowError;

/// Parses a CLI string into a typed `Option<UserRating>`. `Some(_)`
/// for 1-5 / "one"-"five"; `None` for "0" / "none" / "clear" /
/// "remove" (the sentinels for "remove the rating"). Used by the
/// CLI argument path; GUI callers already hold a typed
/// `Option<UserRating>`.
pub fn parse_user_rating(input: &str) -> Result<Option<UserRating>, FicflowError> {
    Ok(match input.to_lowercase().as_str() {
        "1" | "one" => Some(UserRating::One),
        "2" | "two" => Some(UserRating::Two),
        "3" | "three" => Some(UserRating::Three),
        "4" | "four" => Some(UserRating::Four),
        "5" | "five" => Some(UserRating::Five),
        "0" | "none" | "clear" | "remove" => None,
        _ => {
            return Err(FicflowError::InvalidInput(format!(
                "Invalid rating: '{}'. Valid options are numbers 1-5 or words 'one' through 'five', or 'none' to remove rating",
                input
            )));
        }
    })
}

pub fn update_user_rating(
    fanfiction_ops: &dyn FanfictionOps,
    fic_id: u64,
    new_rating: Option<UserRating>,
) -> Result<Fanfiction, FicflowError> {
    let mut fic = fanfiction_ops.get_fanfiction_by_id(fic_id)?;
    fic.user_rating = new_rating;
    fanfiction_ops.save_fanfiction(&fic)?;

    Ok(fic)
}
