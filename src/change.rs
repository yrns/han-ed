use bevy_egui::egui;

pub trait Merge {
    fn merge(self) -> Change;
}

impl Merge for egui::InnerResponse<Change> {
    fn merge(self) -> Change {
        self.inner | self.response
    }
}

// For ComboBox, we only return the item response that's changed, or the header when closed.
impl Merge for egui::InnerResponse<Option<Option<Change>>> {
    fn merge(self) -> Change {
        self.inner.flatten().unwrap_or(self.response.into())
    }
}

// Return the inner response or the header when closed. We don't want the body response since it
// will never be marked changed.
impl Merge for egui::containers::CollapsingResponse<Change> {
    fn merge(self) -> Change {
        //self.body_response.unwrap_or(self.header_response)
        self.body_returned.unwrap_or(self.header_response.into())
    }
}

pub enum Change {
    Change(bool),
    Response(egui::Response),
}

impl Change {
    pub fn changed(&self) -> bool {
        match self {
            Change::Change(c) => *c,
            Change::Response(r) => r.changed(),
        }
    }
}

impl std::ops::BitOr for Change {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        match self {
            Change::Change(c) => match rhs {
                Change::Change(rhs) => Change::Change(c | rhs),
                Change::Response(mut rhs) => Change::Response({
                    if c {
                        rhs.mark_changed()
                    }
                    rhs
                }),
            },
            Change::Response(mut r) => match rhs {
                Change::Change(c) => Change::Response({
                    if c {
                        r.mark_changed()
                    }
                    r
                }),
                Change::Response(rhs) => Change::Response(r | rhs),
            },
        }
    }
}

impl std::ops::BitOr<egui::Response> for Change {
    type Output = Self;

    fn bitor(self, mut rhs: egui::Response) -> Self::Output {
        match self {
            Change::Change(c) => Change::Response({
                if c {
                    rhs.mark_changed();
                }
                rhs
            }),
            Change::Response(r) => Change::Response(r | rhs),
        }
    }
}

impl From<bool> for Change {
    fn from(b: bool) -> Self {
        Change::Change(b)
    }
}

impl From<egui::Response> for Change {
    fn from(r: egui::Response) -> Self {
        Change::Response(r)
    }
}
