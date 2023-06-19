//! egui-datepicker adds a simple date picker widget.
//! Checkout the [example][ex]
//!
//!
//! ```no_run
//! use eframe::egui::Ui;
//! use chrono::prelude::*;
//! use std::fmt::Display;
//! use grus_gui_lib::datepicker::DatePicker;
//! use core::ops::RangeInclusive;
//!
//! struct App
//! {
//!     date: chrono::naive::NaiveDateTime
//! }
//! impl App
//! {
//!     fn draw_datepicker(&mut self, ui: &mut Ui) {
//!         ui.add(DatePicker::<RangeInclusive<NaiveDateTime>>::new("super_unique_id", &mut self.date));
//!     }
//! }
//! ```
//!
//! [ex]: ./examples/simple.rs

use core::ops::{Bound, RangeBounds};
use std::hash::Hash;

pub use chrono::naive::NaiveDateTime;
use chrono::naive::{MAX_DATE, MIN_DATE};
use chrono::{prelude::*, Duration};
use eframe::{
	egui,
	egui::{Area, Color32, DragValue, Frame, Id, Key, Order, Response, RichText, Ui, Widget},
};
use num_traits::FromPrimitive;

/// Default values of fields are:
/// - sunday_first: `false`
/// - movable: `false`
/// - format_string: `"%Y-%m-%d"`
/// - weekend_func: `date.weekday() == Weekday::Sat || date.weekday() == Weekday::Sun`
/// - placment: just underneath the date picker's button
pub struct DatePicker<'a, 'b, R>
where
	R: RangeBounds<NaiveDateTime>,
{
	id: Id,
	date: &'a mut NaiveDateTime,
	sunday_first: bool,
	movable: bool,
	format_string: String,
	weekend_color: Color32,
	weekend_func: fn(&NaiveDateTime) -> bool,
	highlight_weekend: bool,

	// when set, the date picker will restrict dates to the given range.
	allowed_range: Option<&'b R>,
	// when set, will allow place the popup in the given position
	placement: Option<egui::Align2>,

	position_offset: egui::Vec2,
}

impl<'a, 'b, R> DatePicker<'a, 'b, R>
where
	R: RangeBounds<NaiveDateTime>,
{
	/// Create new date picker with unique id and mutable reference to date.
	pub fn new<T: Hash>(id: T, date: &'a mut NaiveDateTime) -> Self {
		Self {
			id: Id::new(id),
			date,
			sunday_first: false,
			movable: false,
			format_string: String::from("%Y-%m-%d"),
			weekend_color: Color32::from_rgb(196, 0, 0),
			weekend_func: |date| date.weekday() == Weekday::Sat || date.weekday() == Weekday::Sun,
			highlight_weekend: true,
			allowed_range: None,
			placement: None,
			position_offset: egui::Vec2 { x: 0., y: 0. },
		}
	}

	/// If flag is set to true then first day in calendar will be sunday otherwise monday.
	/// Default is false
	#[must_use]
	pub fn sunday_first(mut self, flag: bool) -> Self {
		self.sunday_first = flag;
		self
	}

	/// If flag is set to true then date picker popup will be movable.
	/// Default is false
	#[must_use]
	pub fn movable(mut self, flag: bool) -> Self {
		self.movable = flag;
		self.placement = None;
		self
	}

	///Set date format.
	///See the [chrono::format::strftime](https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html) for the specification.
	#[must_use]
	pub fn date_format(mut self, new_format: &impl ToString) -> Self {
		self.format_string = new_format.to_string();
		self
	}

	///If highlight is true then weekends text color will be `weekend_color` instead default text
	///color.
	#[must_use]
	pub fn highlight_weekend(mut self, highlight: bool) -> Self {
		self.highlight_weekend = highlight;
		self
	}

	///Set weekends highlighting color.
	#[must_use]
	pub fn highlight_weekend_color(mut self, color: Color32) -> Self {
		self.weekend_color = color;
		self
	}

	/// Set function, which will decide if date is a weekend day or not.
	pub fn weekend_days(mut self, is_weekend: fn(&NaiveDateTime) -> bool) -> Self {
		self.weekend_func = is_weekend;
		self
	}

	/// The date picker will restrict dates to the given range.
	pub fn restrict_range(mut self, allowed_range: &'b R) -> Self {
		self.allowed_range = Some(allowed_range);
		self
	}

	/// Set a placement for the datepicker to be put.
	/// The window cannot be both placed and movable.
	pub fn placement(mut self, align: egui::Align2) -> Self {
		self.placement = Some(align);
		self.movable = false;
		self
	}

	/// Offset the popup from it's placement by a given amount.
	pub fn position_offset(mut self, offset: impl Into<egui::Vec2>) -> Self {
		self.position_offset = offset.into();
		self
	}

	/// Draw names of week days as 7 columns of grid without calling `Ui::end_row`
	fn show_grid_header(&mut self, ui: &mut Ui) {
		let day_indexes = if self.sunday_first {
			[6, 0, 1, 2, 3, 4, 5]
		} else {
			[0, 1, 2, 3, 4, 5, 6]
		};
		for i in day_indexes {
			let b = Weekday::from_u8(i).unwrap();
			ui.label(b.to_string());
		}
	}

	/// Get number of days between first day of the month and Monday ( or Sunday if field
	/// `sunday_first` is set to `true` )
	fn get_start_offset_of_calendar(&self, first_day: &NaiveDateTime) -> u32 {
		if self.sunday_first {
			first_day.weekday().num_days_from_sunday()
		} else {
			first_day.weekday().num_days_from_monday()
		}
	}

	/// Get number of days between first day of the next month and Monday ( or Sunday if field
	/// `sunday_first` is set to `true` )
	fn get_end_offset_of_calendar(&self, first_day: &NaiveDateTime) -> u32 {
		if self.sunday_first {
			(7 - (first_day).weekday().num_days_from_sunday()) % 7
		} else {
			(7 - (first_day).weekday().num_days_from_monday()) % 7
		}
	}

	fn show_calendar_grid(&mut self, ui: &mut Ui) {
		egui::Grid::new("calendar").min_col_width(30.0).show(ui, |ui| {
			self.show_grid_header(ui);
			let first_day_of_current_month = self.date.with_day(1).unwrap();
			let start_offset = self.get_start_offset_of_calendar(&first_day_of_current_month);
			let days_in_month = get_days_from_month(self.date.year(), self.date.month());
			let first_day_of_next_month =
				first_day_of_current_month + Duration::days(days_in_month);
			let end_offset = self.get_end_offset_of_calendar(&first_day_of_next_month);
			let start_date = first_day_of_current_month - Duration::days(start_offset.into());
			for i in 0..(start_offset as i64 + days_in_month + end_offset as i64) {
				if i % 7 == 0 {
					ui.end_row();
				}
				let d = start_date + Duration::days(i);
				self.show_day_button(d, ui);
			}
		});
	}

	fn show_day_button(&mut self, date: NaiveDateTime, ui: &mut Ui) {
		let mut is_enabled = self.date != &date;

		if let Some(range) = self.allowed_range {
			// round the date up and down to the nearest date
			let day_beginning = date.date().and_hms(0, 0, 0);
			let day_ending = day_beginning + Duration::days(1);

			is_enabled &= range.contains(&day_beginning) | range.contains(&day_ending);
		};

		ui.centered_and_justified(|ui| {
			let mut button = egui::Button::new(date.day().to_string());

			if self.date.month() != date.month() {
				button = button.frame(false);
			} else if self.date == &date {
				// if the date is the selected date,
				// give the button an fill with the 'selection style'
				button = button.fill(ui.style().visuals.selection.bg_fill);
			}
			if self.highlight_weekend && (self.weekend_func)(&date) {
				ui.style_mut().visuals.override_text_color = Some(self.weekend_color);
			}
			if ui.add_enabled(is_enabled, button).clicked() {
				*self.date = date;
			}
		});
	}

	fn show_time_editor(&mut self, ui: &mut Ui) {
		let (hour_range, min_range) = match self.allowed_range {
			Some(range) => {
				let day_beginning = self.date.date().and_hms(0, 0, 0);
				let day_ending = day_beginning + Duration::days(1);

				let (start_hour, start_min) = match range.start_bound() {
					Bound::Included(dt) if day_beginning < *dt => {
						if dt.hour() == self.date.hour() {
							(dt.hour(), dt.minute())
						} else {
							(dt.hour(), 0)
						}
					}
					Bound::Excluded(dt) if day_beginning <= *dt => {
						if dt.hour() == self.date.hour() {
							(dt.hour(), dt.minute() + 1)
						} else {
							(dt.hour(), 0)
						}
					}
					_ => (0, 0),
				};
				let (end_hour, end_min) = match range.end_bound() {
					Bound::Included(dt) if day_ending > *dt => {
						if dt.hour() == self.date.hour() {
							(dt.hour(), dt.minute())
						} else {
							(dt.hour(), 59)
						}
					}
					Bound::Excluded(dt) if day_ending >= *dt => {
						if dt.hour() == self.date.hour() {
							(dt.hour(), dt.minute() - 1)
						} else {
							(dt.hour(), 59)
						}
					}
					_ => (23, 59),
				};

				(start_hour..=end_hour, start_min..=end_min)
			}
			None => (0..=23, 0..=59),
		};

		let curr_hour = self.date.hour() as i64;
		let curr_min = self.date.minute() as i64;
		let mut hour = curr_hour;
		let mut min = curr_min;

		ui.add(egui::DragValue::new(&mut hour).clamp_range(hour_range));
		ui.label(":");
		ui.add(egui::DragValue::new(&mut min).clamp_range(min_range));

		*self.date += Duration::hours(hour - curr_hour) + Duration::minutes(min - curr_min);
	}

	/// Draw current month and buttons for next and previous month.
	fn show_header(&mut self, ui: &mut Ui) {
		ui.horizontal(|ui| {
			self.show_time_editor(ui);
			self.show_month_control(ui);
			self.show_year_control(ui);
		});
	}

	/// Draw button with text and add duration to current date when that button is clicked.
	fn date_step_button(&mut self, ui: &mut Ui, text: impl ToString, duration: Duration) {
		let new_date = *self.date + duration;

		ui.button(text.to_string())
			.clicked()
			.then(|| match self.allowed_range {
				None => *self.date = new_date,
				// Ensure the bound is never left.
				Some(range) => match (range.start_bound(), range.end_bound()) {
					(Bound::Included(lower_b), _) if new_date < *lower_b => *self.date = *lower_b,
					(Bound::Excluded(lower_b), _) if new_date <= *lower_b
						=> *self.date = *lower_b + Duration::nanoseconds(1),

					(_, Bound::Included(upper_b)) if *upper_b < new_date => *self.date = *upper_b,
					(_, Bound::Excluded(upper_b)) if *upper_b <= new_date
						=> *self.date = *upper_b - Duration::nanoseconds(1),

					_ => *self.date = new_date,
				}
			});
	}

	/// Draw drag value widget with current year and two buttons which substract and add 365 days
	/// to current date.
	fn show_year_control(&mut self, ui: &mut Ui) {
		self.date_step_button(ui, "⬅", Duration::days(-365));

		// label used instead of drag value, if there is a range restriction imposed
		// TODO: change clamp_range to enforce imposed range restriction
		if self.allowed_range.is_none() {
			let mut drag_year = self.date.year();
			ui.add(DragValue::new(&mut drag_year).clamp_range(MIN_DATE.year()..=MAX_DATE.year()));

			if drag_year != self.date.year() {
				// `unwrap()` should be safe due to drag value being clamped to the accepted range.
				*self.date = self.date.with_year(drag_year).unwrap();
			}
		} else {
			ui.label(self.date.year().to_string());
		}

		self.date_step_button(ui, "➡", Duration::days(365));
	}

	/// Draw label(will be combobox in future) with current month and two buttons which substract and add 30 days
	/// to current date.
	fn show_month_control(&mut self, ui: &mut Ui) {
		self.date_step_button(ui, "⬅", Duration::days(-30));
		let month_string = &chrono::Month::from_u32(self.date.month()).unwrap().name()[..3];
		// TODO: When https://github.com/emilk/egui/pull/543 is merged try to change label to combo box.
		ui.add(egui::Label::new(
			RichText::new(month_string).text_style(egui::TextStyle::Monospace),
		));
		// let mut selected = self.date.month0() as usize;
		// egui::ComboBox::from_id_source(self.id.with("month_combo_box"))
		//	 .selected_text(selected)
		//	 .show_index(ui, &mut selected, 12, |i| {
		//		 chrono::Month::from_usize(i + 1).unwrap().name().to_string()
		//	 });
		// if selected != self.date.month0() as usize {
		//	 *self.date = self.date.with_month0(selected as u32).unwrap();
		// }
		self.date_step_button(ui, "➡", Duration::days(30));
	}
}

impl<'a, 'b, R> Widget for DatePicker<'a, 'b, R>
where
	R: RangeBounds<NaiveDateTime>,
{
	fn ui(mut self, ui: &mut Ui) -> Response {
		let formated_date = self.date.format(&self.format_string);
		let button_response = ui.button(formated_date.to_string());
		if button_response.clicked() {
			ui.memory_mut(|m| m.toggle_popup(self.id));
		}

		if ui.memory(|m| m.is_popup_open(self.id)) {
			let mut area = Area::new(self.id).order(Order::Foreground).constrain(true);

			if let Some(align) = self.placement {
				area = area.anchor(align, self.position_offset);
			} else {
				area = area.default_pos(button_response.rect.left_bottom() + self.position_offset);
				if !self.movable {
					area = area.movable(false);
				}
			}
			let area_response = area
				.show(ui.ctx(), |ui| {
					Frame::popup(ui.style()).show(ui, |ui| {
						self.show_header(ui);
						self.show_calendar_grid(ui);
					});
				})
				.response;

			if !button_response.clicked()
				&& (ui.input(|i| i.key_pressed(Key::Escape) || area_response.clicked_elsewhere()))
			{
				ui.memory_mut(|m| m.toggle_popup(self.id));
			}
		}
		button_response
	}
}

// https://stackoverflow.com/a/58188385
fn get_days_from_month(year: i32, month: u32) -> i64 {
	NaiveDate::from_ymd(
		match month {
			12 => year + 1,
			_ => year,
		},
		match month {
			12 => 1,
			_ => month + 1,
		},
		1,
	)
	.signed_duration_since(NaiveDate::from_ymd(year, month, 1))
	.num_days()
}
