pub mod clubs;
pub mod feed_card;
pub mod home;
pub mod login;
pub mod otp_password_form;
pub mod people;
pub mod post_detail;
pub mod profile;
pub mod register;
pub mod reset_password;
pub mod settings;

/// Map an exercise type string to a Phosphor icon class and a human-readable label.
///
/// Returns `("ph-medal", "Exercise")` for unknown types.
pub fn exercise_icon_label(exercise_type: &str) -> (&'static str, &'static str) {
    match exercise_type {
        "run" => ("ph-person-simple-run", "Run"),
        "trail-run" => ("ph-person-simple-run", "Trail Run"),
        "virtual-run" => ("ph-person-simple-run", "Virtual Run"),
        "ride" => ("ph-bicycle", "Ride"),
        "gravel-ride" => ("ph-bicycle", "Gravel Ride"),
        "mountain-bike-ride" => ("ph-bicycle", "Mountain Bike"),
        "e-bike-ride" => ("ph-bicycle", "E-Bike Ride"),
        "e-mountain-bike-ride" => ("ph-bicycle", "E-Mountain Bike"),
        "virtual-ride" => ("ph-bicycle", "Virtual Ride"),
        "velomobile" => ("ph-bicycle", "Velomobile"),
        "handcycle" => ("ph-bicycle", "Handcycle"),
        "swim" => ("ph-waves", "Swim"),
        "walk" => ("ph-person-simple-walk", "Walk"),
        "hike" => ("ph-mountains", "Hike"),
        "snowshoe" => ("ph-mountains", "Snowshoe"),
        "alpine-ski" => ("ph-mountains", "Alpine Ski"),
        "backcountry-ski" => ("ph-mountains", "Backcountry Ski"),
        "nordic-ski" => ("ph-mountains", "Nordic Ski"),
        "snowboard" => ("ph-mountains", "Snowboard"),
        "ice-skate" => ("ph-person-simple", "Ice Skate"),
        "inline-skate" => ("ph-person-simple", "Inline Skate"),
        "skateboard" => ("ph-person-simple", "Skateboard"),
        "rowing" => ("ph-boat", "Rowing"),
        "virtual-row" => ("ph-boat", "Virtual Row"),
        "kayaking" => ("ph-boat", "Kayaking"),
        "canoeing" => ("ph-boat", "Canoeing"),
        "stand-up-paddling" => ("ph-waves", "Stand-Up Paddling"),
        "surf" => ("ph-waves", "Surf"),
        "windsurf" => ("ph-waves", "Windsurf"),
        "kitesurf" => ("ph-waves", "Kitesurf"),
        "sail" => ("ph-boat", "Sail"),
        "rock-climbing" => ("ph-mountains", "Rock Climbing"),
        "weight-training" => ("ph-barbell", "Weight Training"),
        "crossfit" => ("ph-barbell", "CrossFit"),
        "hiit" => ("ph-barbell", "HIIT"),
        "elliptical" => ("ph-person-simple", "Elliptical"),
        "stair-stepper" => ("ph-stairs", "Stair Stepper"),
        "yoga" => ("ph-person-simple", "Yoga"),
        "pilates" => ("ph-person-simple", "Pilates"),
        "workout" => ("ph-barbell", "Workout"),
        "golf" => ("ph-golf", "Golf"),
        "soccer" => ("ph-soccer-ball", "Soccer"),
        "tennis" => ("ph-tennis-ball", "Tennis"),
        "squash" => ("ph-tennis-ball", "Squash"),
        "racquetball" => ("ph-tennis-ball", "Racquetball"),
        "badminton" => ("ph-tennis-ball", "Badminton"),
        "pickleball" => ("ph-tennis-ball", "Pickleball"),
        "table-tennis" => ("ph-tennis-ball", "Table Tennis"),
        "wheelchair" => ("ph-wheelchair", "Wheelchair"),
        _ => ("ph-medal", "Exercise"),
    }
}
