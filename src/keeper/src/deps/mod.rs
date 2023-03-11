use crate::{
    settings::Settings,
    storage::{local::LocalStorage, Storage},
};
use common::errors::Result;
use dill::*;

pub fn dependency_injector() -> Result<Catalog> {
    let settings = Settings::new()?;
    let mut builder = CatalogBuilder::new();
    builder.add_builder(
        builder_for::<LocalStorage>().with_settings(settings)
    );
    builder.bind::<dyn Storage, LocalStorage>();
    // builder.with_arg::<LocalStorage, Settings>(settings);

    Ok(builder.build())
}
