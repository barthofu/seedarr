struct Movie {
    title: String,
    year: u16,
    
    video_format: Option<String>, // e.g., x264, x265
    languages: Option<Vec<String>>, // 
    release_group: Option<String>, // e.g., QxR, DIMENSION
    is_hdr: Option<bool>,
    is_10bit: Option<bool>,
    resolution: Option<String>, // e.g., 1080p, 2160p
    source: Option<String>, // e.g., BluRay, WEB-DL
    audio_format: Option<String>, // e.g., DTS, AC3
    audio_channels: Option<String>, // e.g., 5.1, 7.1

    scene_name_from_arr: String,
}

impl Movie {



    
}
