use crate::domain::fanfiction::{DatabaseOps, UserRating};
use std::error::Error;

pub fn update_user_rating(
    db_ops: &dyn DatabaseOps,
    fic_id: u64,
    rating_str: &str
) -> Result<(), Box<dyn Error>> {
    // Get the current fanfiction
    let mut fic = db_ops.get_fanfiction_by_id(fic_id)?;
    
    // Parse the rating string
    let user_rating = match rating_str.to_lowercase().as_str() {
        // Number inputs
        "1" | "one" => Some(UserRating::One),
        "2" | "two" => Some(UserRating::Two),
        "3" | "three" => Some(UserRating::Three),
        "4" | "four" => Some(UserRating::Four),
        "5" | "five" => Some(UserRating::Five),
        // Clear rating
        "0" | "none" | "clear" | "remove" => None,
        _ => return Err(format!("Invalid rating: '{}'. Valid options are numbers 1-5 or words 'one' through 'five', or 'none' to remove rating", rating_str).into())
    };
    
    // Update the user rating
    fic.user_rating = user_rating;
    
    // Update the fanfiction in the database
    db_ops.update_fanfiction(&fic)?;
    
    Ok(())
}
