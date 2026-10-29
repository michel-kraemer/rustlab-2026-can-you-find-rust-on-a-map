use std::{
    fs::File,
    io::{BufReader, Write},
    path::Path,
};

use anyhow::{Context, Result, bail};
use geo::{Centroid, Distance, Euclidean, Geometry, Point};
use geojson::{Feature, GeoJson};
use owo_colors::OwoColorize as _;
use proj::{Proj, Transform};
use rstar::{AABB, RTree, RTreeObject, primitives::GeomWithData};

/// Check if a GeoJSON feature is a possible pirate den
fn has_pirates(feature: &Feature) -> bool {
    feature
        .property("name")
        .and_then(|name| name.as_str())
        .is_some_and(|name| name.to_lowercase().contains("pirate"))
}

/// Check if a GeoJSON feature is an ice cream parlor
fn is_ice_cream_parlor(feature: &Feature) -> bool {
    feature
        .property("amenity")
        .and_then(|name| name.as_str())
        .is_some_and(|name| name == "ice_cream")
}

/// Check if a GeoJSON feature is a place
fn is_place(feature: &Feature) -> bool {
    feature.contains_property("place")
}

/// Run the treasure hunt
pub fn run(input: &Path) -> Result<()> {
    println!("Starting treasure hunt ...");
    println!();

    // Read input. This command expects the FeatureCollection written by
    // `prepare`.
    println!("Reading input file ...");
    let geojson = GeoJson::from_reader(BufReader::new(File::open(input)?))?;
    let GeoJson::FeatureCollection(feature_collection) = geojson else {
        bail!("Input GeoJSON file must contain a FeatureCollection");
    };
    let features = feature_collection.features;

    // divide geometries into pirate dens and ice cream parlors
    let mut pirate_dens = Vec::new();
    let mut ice_cream_parlors = Vec::new();
    let mut places = Vec::new();
    for f in features {
        let geometry: Geometry = f.geometry.as_ref().unwrap().try_into()?;
        if has_pirates(&f)
            && let Geometry::Polygon(polygon) = geometry
        {
            pirate_dens.push(polygon);
        } else if is_ice_cream_parlor(&f)
            && let Geometry::Polygon(polygon) = geometry
        {
            ice_cream_parlors.push(polygon);
        } else if is_place(&f)
            && let Geometry::Point(point) = geometry
        {
            places.push((point, f.properties));
        }
    }

    println!("Found {} possible pirate dens", pirate_dens.len());
    println!("Found {} ice cream parlors", ice_cream_parlors.len());
    println!("Found {} places (cities/towns/villages)", places.len());
    println!();

    // Longitude and latitude are angles on the Earth's surface, so a Euclidean
    // distance computation would return incorrect results. Convert coordinates
    // from WGS84 to ETRS89/UTM zone 32N, which uses meters.
    let from = "EPSG:4326";
    let to = "EPSG:25832";
    let wgs84_to_utm32n = Proj::new_known_crs(from, to, None)?;
    let utm32n_to_wgs84 = Proj::new_known_crs(to, from, None)?;
    pirate_dens
        .iter_mut()
        .try_for_each(|p| p.transform(&wgs84_to_utm32n))?;
    ice_cream_parlors
        .iter_mut()
        .try_for_each(|p| p.transform(&wgs84_to_utm32n))?;
    places
        .iter_mut()
        .try_for_each(|p| p.0.transform(&wgs84_to_utm32n))?;

    // build spatial index for ice cream parlors
    let ice_cream_parlors = RTree::bulk_load(ice_cream_parlors);

    // for each possible pirate den, look for the closest ice cream parlor
    // within a maximum distance of 100m
    let mut min_pirate_den = (f64::MAX, None);
    for p in &pirate_dens {
        let env = p.envelope();
        let expanded = AABB::from_corners(
            Point::new(env.lower().x() - 100.0, env.lower().y() - 100.0),
            Point::new(env.upper().x() + 100.0, env.upper().y() + 100.0),
        );
        for candidate in ice_cream_parlors.locate_in_envelope_intersecting(&expanded) {
            let d = Euclidean.distance(p, candidate);
            if d <= 100.0 && d < min_pirate_den.0 {
                min_pirate_den = (d, Some(p));
            }
        }
    }

    let pirate_den = min_pirate_den
        .1
        .context("no pirate den within 100m of an ice cream parlor")?
        .centroid()
        .unwrap();
    println!(
        "Distance between pirate den and ice cream parlor: {}m",
        min_pirate_den.0
    );
    let pirate_den_wgs84 = utm32n_to_wgs84.convert(pirate_den).unwrap();
    println!(
        "Location of pirate den: {} {}",
        pirate_den_wgs84.y(),
        pirate_den_wgs84.x()
    );

    // build spatial index for places
    let places = RTree::bulk_load(
        places
            .into_iter()
            .map(|(c, props)| GeomWithData::new(c, props))
            .collect(),
    );
    let nearest_place = places
        .nearest_neighbor(&pirate_den)
        .context("no nearest place found")?;

    // print result
    if let Some(serde_json::Value::String(name)) =
        nearest_place.data.as_ref().and_then(|p| p.get("name"))
    {
        let mut stdout = anstream::stdout();
        let msg = format!(
            "┃  Arrrrr! The crate full of {} is in {}!!  ┃",
            "hand-written code".yellow().italic(),
            name.bold().bright_yellow()
        );
        let msg_len = anstream::adapter::strip_str(&msg)
            .to_string()
            .chars()
            .count();
        writeln!(stdout)?;
        writeln!(stdout, "┏{}┓", "━".repeat(msg_len - 2))?;
        writeln!(stdout, "{msg}")?;
        writeln!(stdout, "┗{}┛", "━".repeat(msg_len - 2))?;
    } else {
        println!("We found a nearest place but we don't know its name");
    }

    Ok(())
}
