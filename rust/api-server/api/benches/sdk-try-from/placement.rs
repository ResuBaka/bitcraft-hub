        // Most transactions touch two tables. Place the second table with a
        // single comparison and keep the general merge path for larger sets.
        match self.tables.as_slice() {
            [] => { self.tables.push(update); return; }
            [only] => {
                match update.order().cmp(&only.order()) {
                    std::cmp::Ordering::Less => self.tables.insert(0, update),
                    std::cmp::Ordering::Equal => self.tables[0].append(update),
                    std::cmp::Ordering::Greater => self.tables.push(update),
                }
                return;
            }
            _ => {}
        }
