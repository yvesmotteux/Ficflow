use crate::domain::db::DatabaseOps;
use crate::domain::fic::Fanfiction;
use std::error::Error;

pub fn get_fanfiction(database: &dyn DatabaseOps, fic_id: u64) -> Result<Fanfiction, Box<dyn Error>> {
    database.get_fanfiction_by_id(fic_id)
}

pub fn display_fanfiction_details(fic: &Fanfiction) -> String {
    let mut output = String::new();
    
    // Basic information
    output.push_str(&format!("ID:                  {}\n", fic.id));
    output.push_str(&format!("Title:               {}\n", fic.title));
    output.push_str(&format!("Author(s):           {}\n", fic.authors.join(", ")));
    output.push_str(&format!("Fandom(s):           {}\n", fic.fandoms.join(", ")));
    output.push_str(&format!("Language:            {}\n", fic.language));
    output.push_str(&format!("Rating:              {}\n", fic.rating));
    output.push_str(&format!("Words:               {}\n", fic.words));
    output.push_str(&format!("Chapters:            {}/{}\n", 
        fic.chapters_published, 
        fic.chapters_total.map_or("?".to_string(), |c| c.to_string())
    ));
    output.push_str(&format!("Complete:            {}\n", if fic.complete { "Yes" } else { "No" }));
    output.push_str(&format!("Restricted:          {}\n", if fic.restricted { "Yes" } else { "No" }));
    
    // Stats
    output.push_str(&format!("Hits:                {}\n", fic.hits));
    output.push_str(&format!("Kudos:               {}\n", fic.kudos));
    
    // Dates
    output.push_str(&format!("Published:           {}\n", fic.date_published.format("%Y-%m-%d %H:%M:%S UTC")));
    output.push_str(&format!("Updated:             {}\n", fic.date_updated.format("%Y-%m-%d %H:%M:%S UTC")));
    output.push_str(&format!("Last Checked:        {}\n", fic.last_checked_date.format("%Y-%m-%d %H:%M:%S UTC")));
    
    // Optional fields with conditional formatting
    if let Some(categories) = &fic.categories {
        output.push_str(&format!("Categories:          {}\n", 
            categories.iter().map(|c| format!("{:?}", c)).collect::<Vec<_>>().join(", ")
        ));
    }
    
    if let Some(characters) = &fic.characters {
        output.push_str(&format!("Characters:          {}\n", characters.join(", ")));
    }
    
    if let Some(relationships) = &fic.relationships {
        output.push_str(&format!("Relationships:       {}\n", relationships.join(", ")));
    }
    
    if let Some(tags) = &fic.tags {
        output.push_str(&format!("Tags:                {}\n", tags.join(", ")));
    }
    
    if !fic.warnings.is_empty() {
        output.push_str(&format!("Warnings:            {}\n", 
            fic.warnings.iter().map(|w| format!("{:?}", w)).collect::<Vec<_>>().join(", ")
        ));
    }
    
    // Reading stats
    output.push_str(&format!("Reading Status:      {}\n", fic.reading_status));
    output.push_str(&format!("Read Count:          {}\n", fic.read_count));
    
    if let Some(last_chapter) = fic.last_chapter_read {
        output.push_str(&format!("Last Chapter Read:   {}\n", last_chapter));
    }
    
    if let Some(rating) = &fic.user_rating {
        output.push_str(&format!("Your Rating:         {} / 5\n", *rating as u8));
    }
    
    if let Some(note) = &fic.personal_note {
        output.push_str(&format!("\nPersonal Note:\n{}\n", note));
    }
    
    // Summary at the end
    output.push_str("\nSummary:\n");
    output.push_str(&fic.summary);
    
    output
}