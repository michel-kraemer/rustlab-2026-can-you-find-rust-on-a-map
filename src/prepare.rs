use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use anyhow::Result;
use geo::{Coord, Geometry, LineString, Point, Polygon};
use geojson::{Feature, FeatureCollection, GeoJson, JsonObject};
use osmpbf::{DenseNode, Element, ElementReader, Node, Way};

/// A parsed OpenStreetMap object
struct ParsedObject<T> {
    kind: Kind,
    properties: JsonObject,
    geometry: T,
}

/// The type of a [`ParsedObject`]
enum Kind {
    IceCreamParlor,
    PirateDen,
    Place,
}

/// The location of a [`ParsedObject`]
enum Location {
    /// The object is a way (i.e. polygon) and the coordinates are stored in
    /// nodes with the given IDs.
    ///
    /// An OSM way is an ordered list of node IDs. Its coordinates are resolved
    /// in a second pass. A closed way can then be turned into a polygon.
    Refs(Vec<i64>),

    /// The object is a node with longitude and latitude
    LonLat(f64, f64),
}

/// Run the data preparation
pub fn run(input: &Path, output: &Path) -> Result<()> {
    let objects = find_polygons(input)?;
    write_geojson(objects, output)
}

/// Convert OpenStreetMap tags to GeoJSON properties
fn convert_tags_to_properties<'a, I>(tags: I) -> JsonObject
where
    I: Iterator<Item = (&'a str, &'a str)>,
{
    let mut props = JsonObject::new();
    for (key, value) in tags {
        props.insert(key.to_string(), serde_json::Value::from(value));
    }
    props
}

/// Parse an OpenStreetMap node if it is a "city", "town", or "village". Return
/// an empty Vec if the node is not a "city", "town", or "village".
fn parse_node(node: Node) -> Vec<ParsedObject<Location>> {
    let mut result = Vec::new();

    if node
        .tags()
        .any(|t| t.0 == "place" && (t.1 == "city" || t.1 == "town" || t.1 == "village"))
    {
        result.push(ParsedObject {
            kind: Kind::Place,
            properties: convert_tags_to_properties(node.tags()),
            geometry: Location::LonLat(node.lon(), node.lat()),
        });
    }

    result
}

/// Parse an OpenStreetMap dense node if it is a "city", "town", or "village".
/// Return an empty Vec if the dense node is not a "city", "town", or "village".
fn parse_dense_node(node: DenseNode) -> Vec<ParsedObject<Location>> {
    let mut result = Vec::new();

    if node
        .tags()
        .any(|t| t.0 == "place" && (t.1 == "city" || t.1 == "town" || t.1 == "village"))
    {
        result.push(ParsedObject {
            kind: Kind::Place,
            properties: convert_tags_to_properties(node.tags()),
            geometry: Location::LonLat(node.lon(), node.lat()),
        });
    }

    result
}

/// Parse an OpenStreetMap way if it is a possible pirate den or if it
/// represents an ice cream parlor. Return an empty Vec if it is neither.
fn parse_way(way: Way) -> Vec<ParsedObject<Location>> {
    let mut result = Vec::new();

    if way
        .tags()
        .any(|t| t.0 == "name" && t.1.to_lowercase().contains("pirate"))
    {
        result.push(ParsedObject {
            kind: Kind::PirateDen,
            properties: convert_tags_to_properties(way.tags()),
            geometry: Location::Refs(way.refs().collect()),
        });
    }

    if way.tags().any(|t| t.0 == "amenity" && t.1 == "ice_cream") {
        result.push(ParsedObject {
            kind: Kind::IceCreamParlor,
            properties: convert_tags_to_properties(way.tags()),
            geometry: Location::Refs(way.refs().collect()),
        });
    }

    result
}

/// Read the OpenStreetMap `.osm.pbf` file (a compact binary map format) at the
/// given path, find all pirate dens, ice cream parlors, cities, towns, and
/// villages, and return a Vec of parsed objects with geometries.
fn find_polygons(path: &Path) -> Result<Vec<ParsedObject<Geometry>>> {
    let reader = ElementReader::from_path(path)?;

    // first pass: inspect elements in parallel and collect pirates, ice cream
    // parlors, and places, including the node IDs referenced by ways
    let objs_with_refs = reader.par_map_reduce(
        |e| match e {
            Element::Node(node) => parse_node(node),
            Element::DenseNode(node) => parse_dense_node(node),
            Element::Way(way) => parse_way(way),
            _ => Vec::new(),
        },
        Vec::new,
        |mut a, b| {
            a.extend(b);
            a
        },
    )?;

    // gather the node IDs referenced by the selected ways so the second pass
    // only needs to keep coordinates that will be used
    let node_ids_to_find = objs_with_refs
        .iter()
        .filter_map(|obj| match &obj.geometry {
            Location::Refs(items) => Some(items),
            _ => None,
        })
        .flatten()
        .copied()
        .collect::<HashSet<_>>();

    // second pass: resolve referenced node IDs to longitude/latitude pairs
    let reader = ElementReader::from_path(path)?;
    let nodes_to_coordinates = reader
        .par_map_reduce(
            |e| match e {
                Element::Node(node) if node_ids_to_find.contains(&node.id()) => {
                    vec![(node.id(), Coord::from((node.lon(), node.lat())))]
                }
                Element::DenseNode(node) if node_ids_to_find.contains(&node.id()) => {
                    vec![(node.id(), Coord::from((node.lon(), node.lat())))]
                }
                _ => Vec::new(),
            },
            Vec::new,
            |mut a, b| {
                a.extend(b);
                a
            },
        )?
        .into_iter()
        .collect::<HashMap<_, _>>();

    // Turn the saved locations into geo crate geometries. For ways, use the
    // available coordinates and keep only non-empty, closed rings.
    Ok(objs_with_refs
        .into_iter()
        .filter_map(|obj| match obj.geometry {
            Location::Refs(refs) => {
                let coords = refs
                    .into_iter()
                    .filter_map(|id| nodes_to_coordinates.get(&id).copied())
                    .collect::<Vec<_>>();
                if !coords.is_empty() && coords[0] == coords[coords.len() - 1] {
                    Some(ParsedObject {
                        kind: obj.kind,
                        properties: obj.properties,
                        geometry: Polygon::new(LineString::new(coords), Vec::new()).into(),
                    })
                } else {
                    None
                }
            }
            Location::LonLat(lon, lat) => Some(ParsedObject {
                kind: obj.kind,
                properties: obj.properties,
                geometry: Point::new(lon, lat).into(),
            }),
        })
        .collect())
}

/// Write the given parsed objects into a GeoJSON file
fn write_geojson(objects: Vec<ParsedObject<Geometry>>, output: &Path) -> Result<()> {
    let features = objects
        .into_iter()
        .map(|o| {
            let geom: geojson::Geometry = (&o.geometry).into();
            Feature {
                geometry: Some(geom),
                properties: Some(o.properties),
                ..Default::default()
            }
        })
        .collect::<Vec<_>>();

    let fc = FeatureCollection {
        features,
        ..Default::default()
    };

    std::fs::write(output, GeoJson::FeatureCollection(fc).to_string_pretty()?)?;

    Ok(())
}
