use std::ops::{Deref, DerefMut};
use thiserror::Error;
use tokio::sync::mpsc;

pub struct RecycleHolder<'a, T> {
    not_consumed: &'a mut Option<T>,
    obj: Option<T>,
    tx: mpsc::Sender<T>,
}

impl<'a, T> RecycleHolder<'a, T> {
    pub fn new(
        not_consumed: &'a mut Option<T>,
        obj: Option<T>,
        tx: mpsc::Sender<T>,
    ) -> RecycleHolder<'a, T> {
        RecycleHolder {
            not_consumed,
            obj,
            tx,
        }
    }

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
