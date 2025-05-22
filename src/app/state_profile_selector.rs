#[derive(Debug, Default, Clone)]
pub struct ProfileSelectorState {
    pub is_active: bool,
    pub selected_index: usize,
}

impl ProfileSelectorState {
    pub fn toggle(&mut self, current_profile_index: usize) {
        self.is_active = !self.is_active;
        if self.is_active {
            self.selected_index = current_profile_index;
        }
    }

    pub fn next(&mut self, profiles_len: usize) {
        if profiles_len > 0 {
            self.selected_index = (self.selected_index + 1) % profiles_len;
        }
    }

    pub fn previous(&mut self, profiles_len: usize) {
        if profiles_len > 0 {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = profiles_len - 1;
            }
        }
    }
}
