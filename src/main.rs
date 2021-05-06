#[macro_use]
extern crate clap;
extern crate csv;

use rand::distributions::{Distribution, WeightedIndex};
use rand::seq::IteratorRandom;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;

//Represents a single Song
#[derive(PartialEq, Eq, Hash, Clone, Debug, Deserialize)]
struct Song {
    track: String,
    artist: String,
    album: String,
}

//Set up clap and parse command line arguments
fn parse_args() -> (String, i32, f32, bool) {
    let matches = clap_app!(rplaylist =>
        (version: "0.1.0")
        (author: "github.com/e-dm-b")
        (about: "Uses a modified Markov chain to generate a playlist based on Last.fm listening history")
        (@arg INPUT: +required "Sets the input file to use")
        (@arg LENGTH: -l --length +takes_value "Sets the number of songs in the generated playlist")
        (@arg CREATIVITY: -c --creativity +takes_value "Sets the playlist generation creativity")
        (@arg verbose: -v --verbose ... "Prints verbose information on song probabilities. Useful for fine-tuning creativity")
    )
    .get_matches();

    let input_file_path = matches.value_of("INPUT").unwrap();

    let playlist_length = match matches.value_of("LENGTH").unwrap_or("20").parse::<i32>() {
        Ok(playlist_length) => playlist_length,
        Err(_e) => 20, //if playlist_length cannot be parsed set to 20
    };

    let creativity = match matches.value_of("CREATIVITY").unwrap_or("0").parse::<f32>() {
        Ok(creativity) => creativity,
        Err(_e) => 0.0, //if creativity cannot be parsed set to 0.0
    };

    let verbosity = matches.is_present("verbose");

    (
        input_file_path.to_string(),
        playlist_length,
        creativity,
        verbosity,
    )
}

//Reads the input file and populates the suppied Vec of Songs, with the first Song being the last in the file
fn read_songs(input_file: &File, songs_list: &mut Vec<Song>) -> Result<(), Box<dyn Error>> {
    let mut reader = csv::Reader::from_reader(input_file);
    for row in reader.deserialize() {
        let record: Song = row?;
        songs_list.insert(0, record);
    }
    Ok(())
}

//Returns a random Song selected from the list of unique Songs
fn random_song(uniques: &HashMap<Song, HashMap<Song, f32>>) -> Song {
    uniques
        .keys()
        .into_iter()
        .choose(&mut rand::thread_rng())
        .unwrap()
        .clone()
}

//Selects a Song given a HashMap of Songs and their probabilities
fn choose_by_prob(probabilities: &HashMap<Song, f32>, verbose: bool) -> Song {
    let mut songs: Vec<Song> = Vec::new();
    let mut weights: Vec<f32> = Vec::new();

    for (s, p) in probabilities.clone().drain() {
        songs.push(s);
        weights.push(p);
    }
    let distribution = WeightedIndex::new(&weights).unwrap();

    if verbose {
        for i in 0..songs.len() {
            println!(
                "\t\t{} - {} = {}",
                songs.get(i).unwrap().artist,
                songs.get(i).unwrap().track,
                weights.get(i).unwrap()
            );
        }
    }

    songs
        .get(distribution.sample(&mut rand::thread_rng()))
        .unwrap()
        .clone()
}

//Predicts the next Song given the current Song and a list of all Songs and their potential next songs
fn predict_next(
    current_song: &Song,
    uniques: &HashMap<Song, HashMap<Song, f32>>,
    verbose: bool,
) -> Song {
    let next_songs_opt = uniques.get(current_song);
    if next_songs_opt.is_some() && next_songs_opt.unwrap().len() != 0 {
        return choose_by_prob(next_songs_opt.unwrap(), verbose);
    }
    //Couldn't find current_song in uniques, or current_song has no possible next song
    random_song(uniques) //So, return a random song instead
}

fn main() {
    let (input_file_path, playlist_length, creativity, verbose) = parse_args();

    if verbose {
        println!("Using input file {}\nUsing playlist length {}\nUsing creativity {}\n",
               input_file_path, playlist_length, creativity);
    }

    //open input file
    let input_file = match File::open(input_file_path.clone()) {
        Ok(input_file) => input_file,
        Err(_e) => {
            println!("Failed to open input file {}", input_file_path);
            std::process::exit(1);
        }
    };

    //read csv rows to a vector
    let mut all_songs = Vec::<Song>::new();
    if let Err(err) = read_songs(&input_file, &mut all_songs) {
        println!("{}", err);
        std::process::exit(1);
    }

    //generate HashMap of unique songs and HashMaps of Songs and counts
    //outer HashMap contains every unique Song as the keys and inner HashMaps as the values
    //inner HashMaps contain every Song following the key Song, and how many times they occur
    let mut unique_songs: HashMap<Song, HashMap<Song, f32>> = HashMap::new();
    for i in 0..all_songs.len() - 1 {
        let current_song: Song = all_songs.get(i).cloned().unwrap();
        let next_song: Song = all_songs.get(i + 1).cloned().unwrap();

        let mut next_songs: HashMap<Song, f32> = HashMap::new(); // create inner HashMap
        next_songs.insert(next_song.clone(), 0.0);

        let current_song_map = unique_songs.entry(current_song).or_insert(next_songs);

        *current_song_map.entry(next_song).or_insert(0.0) += 1.0;
    }

    //Apply creativity to counts
    //If count is below average for all possible songs, add average * creativity to it
    //If count is above average for all possible songs, subtract average * creativity from it
    for (_key_song, following_songs) in unique_songs.iter_mut() {
        let mut row_total: f32 = 0.0;
        for (_next_song, count) in following_songs.iter() {
            row_total += *count;
        }
        let row_average: f32 = row_total / following_songs.len() as f32;

        for (_next_song, count) in following_songs.iter_mut() {
            if *count < row_average {
                *count += row_average * creativity;
            } else if *count > row_average {
                *count -= row_average * creativity;
            }
            if *count < 1 as f32 {
                //clamp counts to avoid negatives
                *count = 1 as f32;
            }
        }
    }

    //choose a random song, then use it to seed playlist generation
    let mut current_song = random_song(&unique_songs);
    println!("1.\t{} - {}", current_song.artist, current_song.track);

    for i in 2..=playlist_length {
        current_song = predict_next(&current_song, &unique_songs, verbose);
        println!("{}.\t{} - {}", i, current_song.artist, current_song.track);
    }
}
