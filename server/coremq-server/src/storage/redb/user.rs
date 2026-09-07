use std::sync::Arc;

use anyhow::Result;

use crate::cluster::meta::Replication;
use crate::cluster::protocol::MetaTable;
use redb::{Database, ReadableTable, TableDefinition};

use crate::{enums::role::RoleType, models::user::User, utils};

pub const USERS: TableDefinition<&str, &[u8]> = TableDefinition::new("users");
static DEFAULT_PASSWORD: &str = "public";

#[derive(Clone)]
pub struct UserRepo {
    db: Arc<Database>,
    replication: Option<Arc<Replication>>,
}

impl UserRepo {
    /*
      Attach the cluster replication hook. Absent on a single-node broker, in
      which case writes are neither stamped nor announced.
    */
    pub fn set_replication(&mut self, replication: Option<Arc<Replication>>) {
        self.replication = replication;
    }

    pub fn new(db: Arc<Database>) -> Self {
        let write_txn = db
            .begin_write()
            .expect("Failed to begin write txn for table init");
        let _ = write_txn
            .open_table(USERS)
            .expect("Failed to create/open USERS table");
        write_txn.commit().expect("Failed to commit table init");
        let repo = Self { db, replication: None };
        repo.ensure_admin().expect("Failed to ensure admin user");
        repo
    }

    pub fn create(&self, user: &User) -> Result<()> {
        let bytes = bincode::serialize(user)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(USERS)?;
            table.insert(user.username.as_str(), bytes.as_slice())?;
        }
        write_txn.commit()?;

        if let Some(r) = &self.replication {
            r.record(MetaTable::User, &user.username, Some(bytes));
        }
        Ok(())
    }

    pub fn get(&self, username: &str) -> Result<Option<User>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(USERS)?;
        if let Some(value) = table.get(username)? {
            let user: User = bincode::deserialize(value.value())?;
            Ok(Some(user))
        } else {
            Ok(None)
        }
    }

    pub fn delete(&self, username: &str) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(USERS)?;
            table.remove(username)?;
        }
        write_txn.commit()?;

        if let Some(r) = &self.replication {
            r.record(MetaTable::User, username, None);
        }
        Ok(())
    }

    pub fn get_all(&self) -> Result<Vec<User>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(USERS)?;

        let mut users = Vec::new();

        for entry in table.iter()? {
            let (_key, value) = entry?;
            let user: User = bincode::deserialize(value.value())?;
            users.push(user);
        }

        Ok(users)
    }

    fn ensure_admin(&self) -> Result<()> {
        if self.get("admin")?.is_some() {
            return Ok(());
        }

        let hashed = utils::password::hash_password(DEFAULT_PASSWORD).unwrap();

        let admin = User {
            username: "admin".to_string(),
            password_hash: hashed,
            role: RoleType::User.to_string(),
        };

        self.create(&admin)?;

        println!("Default admin user created");

        Ok(())
    }
}
