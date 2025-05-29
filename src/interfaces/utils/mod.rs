pub mod formatter {
    pub fn format_word_count(words: u32) -> String {
        let mut result = String::new();
        let words_str = words.to_string();
        let len = words_str.len();
        
        for (i, c) in words_str.chars().enumerate() {
            if i > 0 && (len - i) % 3 == 0 {
                result.push(',');
            }
            result.push(c);
        }
        
        result
    }
}