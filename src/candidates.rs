use crate::storage::Storage;
use anyhow::Result;
use std::{collections::HashMap, io::Write};

pub fn write(query: &str, mut output: impl Write) -> Result<()> {
    let storage = Storage::discover()?;
    storage.tokens()?;
    let snapshot = storage.snapshot()?;
    let list = snapshot.current_list()?;
    let mut aliases: Vec<_> = storage.aliases()?.into_iter().collect();
    aliases.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let aliases: HashMap<_, _> = aliases
        .into_iter()
        .rev()
        .map(|(alias, item_id)| (item_id, alias))
        .collect();
    let query = query.trim().to_lowercase();

    writeln!(output, "{}", list.name)?;
    for item in &list.items {
        let alias = aliases.get(&item.id).map_or("", String::as_str);
        if [item.id.as_str(), alias, &item.name, &item.notes]
            .iter()
            .any(|field| field.to_lowercase().contains(&query))
        {
            writeln!(
                output,
                "{}\t{alias}\t{}\t{}",
                item.id, item.name, item.notes
            )?;
        }
    }
    Ok(())
}
