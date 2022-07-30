use handlebars::{Context, Handlebars, Helper, HelperDef, RenderContext, RenderError, ScopedJson};
use log::debug;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::{fs, io};

pub(crate) struct ExifHelper;

impl HelperDef for ExifHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        _: &Helper<'reg, 'rc>,
        _: &'reg Handlebars,
        context: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'reg, 'rc>, RenderError> {
        let path = context.data().get("__self").unwrap().as_str().unwrap();

        debug!("Got path from context: {path:?}");

        let exif = {
            let mut self_file = match fs::File::open(path) {
                Ok(file) => io::BufReader::new(file),
                Err(_) => return Ok(ScopedJson::Derived(json!(null))),
            };

            match exif::Reader::new().read_from_container(&mut self_file) {
                Ok(exif) => exif,
                Err(_) => {
                    debug!("Couldn't read exif from {path:?}");
                    return Ok(ScopedJson::Derived(json!(null)));
                }
            }
        };

        let fields: HashMap<String, Value> = exif
            .fields()
            .map(|field| {
                let name = field.tag.to_string();
                let value = format!("{}", field.display_value());
                (name, value.into())
            })
            .collect();

        return Ok(ScopedJson::Derived(json!(fields)));
    }
}
