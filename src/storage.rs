
use std;
use std::io;
use std::io::ErrorKind;
use mysql;
use mysql::conn::IsolationLevel;

// Track both our transaction state and our data in our mail type
pub enum Mail {
    Empty,
    WithFrom(String),
    WithTo { from: String, tos: Vec<String> },
    WithData { from: String, tos: Vec<String>, data: String },
}


pub trait Storage {
    fn store_mail(&self, mail: &Mail) -> io::Result<()>;
}

pub struct DbStorage {
    pool: mysql::Pool
}

impl DbStorage {
    pub fn new(pool: mysql::Pool) -> DbStorage {
        DbStorage { pool: pool }
    }
}

impl Storage for DbStorage {
    fn store_mail(&self, mail: &Mail) -> io::Result<()> {
        if let &Mail::WithData { ref from, ref tos, ref data } = mail {
            let tx_result = self.pool.start_transaction(true, Some(IsolationLevel::RepeatableRead), None)
            .and_then(|mut tx| {
                // we have to map on this result to allow it to go out of
                // scope
                // we just keep the insert ID
                let mail_id_res =
                    tx.prep_exec("INSERT INTO mail
                                    (`from`, compressed_data)
                                  VALUES
                                    (:from, compress(:data))",
                                    params!{ "from" => from, "data" => data })
                    .map(|result| result.last_insert_id());

                mail_id_res.and_then(|mail_id| {
                    let mut end_result = Ok(());
                    for mut stmt in tx.prepare("INSERT INTO rcpt
                                                (mail_id, rcpt)
                                                VALUES
                                                (:mail_id, :rcpt)").into_iter() {
                        for to_addr in tos.iter() {
                            end_result = end_result.and(
                                stmt.execute(params!{
                                    "mail_id" => mail_id,
                                    "rcpt" => to_addr
                                }).map(|_| ())
                            );
                        }
                    }
                    end_result.and_then(|_| {
                        tx.commit()
                    })
                })
            });
            match tx_result {
                Ok(value) => {
                    println!("Successfully inserted mail");
                    Ok(value)
                }
                Err(err) => {
                    let e: &std::error::Error = &err;
                    println!("Failed to mail: {}", e.description());

                    Err(io::Error::new(ErrorKind::Other, e.description().to_owned()))
                }
            }
        } else {
            panic!("programming error: must store complete mail");
        }
    }
}


