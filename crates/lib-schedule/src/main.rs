// use chrono::{DateTime, Duration, Local, LocalResult, NaiveDateTime, TimeZone, Utc};
// use chrono_tz::{America::New_York, Australia::Sydney, OffsetComponents, OffsetName};

// fn main() {
//     let spring_forward = NaiveDateTime::parse_from_str("2023-03-12 02:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
//     let fall_back = NaiveDateTime::parse_from_str("2023-11-05 01:30:00", "%Y-%m-%d %H:%M:%S").unwrap();

//     // Ambiguous time during "fall back"
//     let new_york_time = New_York.from_local_datetime(&fall_back).single();
//     match new_york_time {
//         Some(time) => println!("Un Ambiguous time: {}", time),
//         None => {
//             match New_York.offset_from_local_datetime(&fall_back) {
//                 chrono::LocalResult::None => todo!(),
//                 chrono::LocalResult::Single(x) => println!("Ambiguous time: {}", x),
//                 chrono::LocalResult::Ambiguous(y, z) => println!("Ambiguous time: {:?} ", New_York.from_local_datetime(&fall_back).earliest()),
//             }
//         },
//     }

// New_York.from_local_datetime(&spring_forward).single();

// let new_york_time = New_York.from_local_datetime(&spring_forward).single();
// match new_york_time {
//     Some(time) => println!("Un Ambiguous time: {}", time),
//     None => {
//        match New_York.offset_from_local_datetime(&spring_forward) {
//             chrono::LocalResult::None => todo!(),
//             chrono::LocalResult::Single(x) => println!("Ambiguous time: {}", x),
//             chrono::LocalResult::Ambiguous(y, z) => println!("Ambiguous time: {} or {}", y, z),
//         }
//     },
// }

// // Add one hour
// let new_time = new_york_time + Duration::hours(2);

// println!("Original time: {}", new_york_time);
// println!("Time after adding 1 hour: {}", new_time);
// }

// use chrono::{NaiveDateTime, TimeZone, LocalResult};
// use chrono_tz::America::New_York;

// use chrono::NaiveTime;

// fn main() {
//     let time1 = NaiveTime::from_hms(23, 0, 0); // 11 PM
//     let time2 = time1 + chrono::Duration::hours(2); // Add 2 hours

//     println!("Time: {}", time2); // Output: Time: 01:00:00
// }

// fn main() {
//     let naive_datetime = NaiveDateTime::parse_from_str("2023-03-12 02:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
//     // Use LocalResult to handle the ambiguous time
//     match New_York.from_local_datetime(&naive_datetime) {
//         LocalResult::None => println!("Spring forward: {:?}", New_York.from_local_datetime(&naive_datetime).earliest()),
//         LocalResult::Single(datetime) => {
//             println!("Unambiguous time: {}", datetime);
//         }
//         LocalResult::Ambiguous(earlier, later) => {
//             // Decide whether to use earlier or later based on your application logic
//             // For "fall back", typically use earlier:
//             println!("Ambiguous time (fall back): {}", earlier);

//             // For "spring forward", typically use later:
//             // println!("Ambiguous time (spring forward): {}", later);
//         }
//     }
// }

fn main() {}
