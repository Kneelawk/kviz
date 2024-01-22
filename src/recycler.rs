use std::ops::{Deref, DerefMut};
use thiserror::Error;
use tokio::sync::mpsc;

pub struct RecycleProducer<T: Send> {
    not_consumed: Option<T>,
    recycle_rx: mpsc::Receiver<T>,
    data_tx: mpsc::Sender<T>,
}

pub struct RecycleConsumer<T: Send> {
    not_consumed: Option<T>,
    data_rx: mpsc::Receiver<T>,
    recycle_tx: mpsc::Sender<T>,
}

pub async fn recycler<T: Send>(objs: Vec<T>) -> (RecycleProducer<T>, RecycleConsumer<T>) {
    let (data_tx, data_rx) = mpsc::channel(objs.len());
    let (recycle_tx, recycle_rx) = mpsc::channel(objs.len());

    for obj in objs {
        recycle_tx.send(obj).await.unwrap();
    }

    (
        RecycleProducer {
            not_consumed: None,
            recycle_rx,
            data_tx,
        },
        RecycleConsumer {
            not_consumed: None,
            data_rx,
            recycle_tx,
        },
    )
}

impl<T: Send> RecycleProducer<T> {
    pub fn recv_recycling_blocking(&mut self) -> Option<RecycleHolder<T>> {
        if let Some(obj) = self.not_consumed.take() {
            return Some(RecycleHolder {
                not_consumed: &mut self.not_consumed,
                obj: Some(obj),
                tx: self.data_tx.clone(),
            });
        }

        if let Some(obj) = self.recycle_rx.blocking_recv() {
            return Some(RecycleHolder {
                not_consumed: &mut self.not_consumed,
                obj: Some(obj),
                tx: self.data_tx.clone(),
            });
        }

        None
    }
}

impl<T: Send> RecycleConsumer<T> {
    pub async fn recv_data(&mut self) -> Option<RecycleHolder<T>> {
        if let Some(obj) = self.not_consumed.take() {
            return Some(RecycleHolder {
                not_consumed: &mut self.not_consumed,
                obj: Some(obj),
                tx: self.recycle_tx.clone(),
            });
        }

        if let Some(obj) = self.data_rx.recv().await {
            return Some(RecycleHolder {
                not_consumed: &mut self.not_consumed,
                obj: Some(obj),
                tx: self.recycle_tx.clone(),
            });
        }

        None
    }
}

pub struct RecycleHolder<'a, T> {
    not_consumed: &'a mut Option<T>,
    obj: Option<T>,
    tx: mpsc::Sender<T>,
}

impl<'a, T> RecycleHolder<'a, T> {
    pub async fn send(&mut self) -> Result<(), RecycleError> {
        if let Some(obj) = self.obj.take() {
            match self.tx.send(obj).await {
                Ok(()) => {}
                Err(mpsc::error::SendError(obj)) => {
                    *self.not_consumed = Some(obj);
                    return Err(RecycleError::SendError);
                }
            };
        } else {
            return Err(RecycleError::AlreadySent);
        }

        Ok(())
    }

    pub fn blocking_send(&mut self) -> Result<(), RecycleError> {
        if let Some(obj) = self.obj.take() {
            match self.tx.blocking_send(obj) {
                Ok(()) => {}
                Err(mpsc::error::SendError(obj)) => {
                    *self.not_consumed = Some(obj);
                    return Err(RecycleError::SendError);
                }
            };
        } else {
            return Err(RecycleError::AlreadySent);
        }

        Ok(())
    }
}

impl<'a, T> Deref for RecycleHolder<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.obj
            .as_ref()
            .expect("Attempt to deref recycle holder after it has been sent")
    }
}

impl<'a, T> DerefMut for RecycleHolder<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.obj
            .as_mut()
            .expect("Attempt to deref recycle holder after it has been sent")
    }
}

impl<'a, T> Drop for RecycleHolder<'a, T> {
    fn drop(&mut self) {
        *self.not_consumed = self.obj.take();
    }
}

#[derive(Debug, Error)]
pub enum RecycleError {
    #[error("Error sending holder contents")]
    SendError,
    #[error("Holder contents have already been sent")]
    AlreadySent,
}
