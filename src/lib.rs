pub mod datepicker;
mod grid;
mod layout;
mod placer;

use egui::{pos2, Align, Color32, Context, Direction, FontSelection, NumExt, Pos2, Rect, Response, Rounding, Sense, Shape, Style, TextureId, Ui, Vec2, WidgetText};
use egui::style::TextStyle;
use egui::widgets::Image;
use egui::widget_text::WidgetTextGalley;
use epaint::{Stroke, TextShape};
use layout::Layout;
use placer::Placer;

pub struct WidgetPlacer<'ui> {
	placer: Placer,
	pub style: &'ui Style,
	context: &'ui Context,
}

impl<'ui> WidgetPlacer<'ui> {
	pub fn new(ui: &'ui Ui) -> Self {
		WidgetPlacer {
			placer: Placer::new(ui.available_rect_before_wrap(), ui.layout().clone().into()),
			style: ui.style(),
			context: ui.ctx(),
		}
	}
	/// Returns a [`Rect`] with exactly what you asked for.
	///
	/// The response rect will be larger if this is part of a justified layout or similar.
	/// This means that if this is a narrow widget in a wide justified layout, then
	/// the widget will react to interactions outside the returned [`Rect`].
	pub fn allocate_exact_size(&mut self, desired_size: Vec2) -> (Rect, Rect) {
		let response_rect = self.allocate_space(desired_size);
		let rect = self
			.placer
			.align_size_within_rect(desired_size, response_rect);
		(rect, response_rect)
	}

	/// Reserve this much space and move the cursor.
	/// Returns where to put the widget.
	fn allocate_space(&mut self, desired_size: Vec2) -> Rect {
		let item_spacing = self.style.spacing.item_spacing;
		let frame_rect = self.placer.next_space(desired_size, item_spacing);
		egui::egui_assert!(!frame_rect.any_nan());
		let widget_rect = self.placer.justify_and_align(frame_rect, desired_size);

		self.placer
			.advance_after_rects(frame_rect, widget_rect, item_spacing);

		widget_rect
	}

	pub(crate) fn allocate_rect(&mut self, rect: Rect) {
		egui::egui_assert!(!rect.any_nan());
		let item_spacing = self.style.spacing.item_spacing;
		self.placer.advance_after_rects(rect, rect, item_spacing);
	}

	/// Where do we expect a zero-sized widget to be placed?
	pub fn next_widget_position(&self) -> Pos2 {
		self.placer.next_widget_position()
	}

	/// Add extra space before the next widget.
	///
	/// The direction is dependent on the layout.
	/// This will be in addition to the [`crate::style::Spacing::item_spacing`].
	///
	/// [`Self::min_rect`] will expand to contain the space.
	#[inline]
	pub fn add_space(&mut self, amount: f32) {
		self.placer.advance_cursor(amount);
	}

	pub fn allocate_ui_with_layout<R>(
		&mut self,
		desired_size: Vec2,
		layout: Layout,
		add_contents: impl FnOnce(&mut Self) -> R,
	) -> (R, Rect) {
		egui::egui_assert!(desired_size.x >= 0.0 && desired_size.y >= 0.0);
		let item_spacing = self.style.spacing.item_spacing;
		let frame_rect = self.placer.next_space(desired_size, item_spacing);
		let child_rect = self.placer.justify_and_align(frame_rect, desired_size);

		let mut child_wp = WidgetPlacer {
			placer: Placer::new(child_rect, layout),
			style: self.style,
			context: self.context,
		};
		let ret = add_contents(&mut child_wp);
		let final_child_rect = child_wp.placer.min_rect();

		self.placer.advance_after_rects(final_child_rect, final_child_rect, item_spacing);

		(ret, final_child_rect)
	}

	/// Layout with wrap mode based on the containing [`Ui`].
	///
	/// wrap: override for [`Ui::wrap_text`].
	pub fn into_galley(
		&self,
		text: WidgetText,
		wrap: Option<bool>,
		available_width: f32,
		fallback_font: impl Into<FontSelection>,
	) -> WidgetTextGalley {
		let wrap = wrap.unwrap_or_else(|| self.wrap_text());
		let wrap_width = if wrap { available_width } else { f32::INFINITY };

		match text {
			WidgetText::RichText(text) => {
				let valign = self.placer.layout().vertical_align();
				let mut text_job = WidgetText::RichText(text).into_text_job(self.style, fallback_font.into(), valign);
				text_job.job.wrap.max_width = wrap_width;
				WidgetTextGalley {
					galley: self.context.fonts(|f| f.layout_job(text_job.job)),
					galley_has_color: text_job.job_has_color,
				}
			}
			WidgetText::LayoutJob(mut job) => {
				job.wrap.max_width = wrap_width;
				WidgetTextGalley {
					galley: self.context.fonts(|f| f.layout_job(job)),
					galley_has_color: true,
				}
			}
			WidgetText::Galley(galley) => WidgetTextGalley {
				galley,
				galley_has_color: true,
			},
		}
	}

	/// Should text wrap in this [`Ui`]?
	///
	/// This is determined first by [`Style::wrap`], and then by the layout of this [`Ui`].
	pub fn wrap_text(&self) -> bool {
		if let Some(wrap) = self.style.wrap {
			wrap
		} else if let Some(grid) = self.placer.grid() {
			grid.wrap_text()
		} else {
			let layout = self.placer.layout();
			layout.is_vertical() || layout.is_horizontal() && layout.main_wrap()
		}
	}
}

pub trait ExtLayout {
	fn left_to_right<R>(&mut self, add_contents: impl FnOnce(&mut WidgetPlacer) -> R) -> (R, Rect);
	fn right_to_left<R>(&mut self, add_contents: impl FnOnce(&mut WidgetPlacer) -> R) -> (R, Rect);
}

impl ExtLayout for WidgetPlacer<'_> {
	fn left_to_right<R>(&mut self, add_contents: impl FnOnce(&mut WidgetPlacer) -> R) -> (R, Rect) {
		let initial_size = Vec2::new(
			self.placer.available_rect_before_wrap().size().x,
			self.style.spacing.interact_size.y,
		);
		self.allocate_ui_with_layout(initial_size, Layout::left_to_right(Align::Center).into(), add_contents)
	}

	fn right_to_left<R>(&mut self, add_contents: impl FnOnce(&mut WidgetPlacer) -> R) -> (R, Rect) {
		let initial_size = Vec2::new(
			self.placer.available_rect_before_wrap().size().x,
			self.style.spacing.interact_size.y,
		);
		self.allocate_ui_with_layout(initial_size, Layout::right_to_left(Align::Center).into(), add_contents)
	}
}

/// Static text.
///
/// Usually it is more convenient to use [`Ui::label`].
///
/// ```
/// # egui::__run_test_ui(|ui| {
/// ui.label("Equivalent");
/// ui.add(egui::Label::new("Equivalent"));
/// ui.add(egui::Label::new("With Options").wrap(false));
/// ui.label(egui::RichText::new("With formatting").underline());
/// # });
/// ```
#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct Label {
	text: WidgetText,
	wrap: Option<bool>,
	sense: Option<Sense>,
}

impl Label {
	pub fn new(text: impl Into<WidgetText>) -> Self {
		Self {
			text: text.into(),
			wrap: None,
			sense: None,
		}
	}

	pub fn text(&self) -> &str {
		self.text.text()
	}

	/// If `true`, the text will wrap to stay within the max width of the [`Ui`].
	///
	/// By default [`Self::wrap`] will be `true` in vertical layouts
	/// and horizontal layouts with wrapping,
	/// and `false` on non-wrapping horizontal layouts.
	///
	/// Note that any `\n` in the text will always produce a new line.
	///
	/// You can also use [`crate::Style::wrap`].
	#[inline]
	pub fn wrap(mut self, wrap: bool) -> Self {
		self.wrap = Some(wrap);
		self
	}

	/// Make the label respond to clicks and/or drags.
	///
	/// By default, a label is inert and does not respond to click or drags.
	/// By calling this you can turn the label into a button of sorts.
	/// This will also give the label the hover-effect of a button, but without the frame.
	///
	/// ```
	/// # use egui::{Label, Sense};
	/// # egui::__run_test_ui(|ui| {
	/// if ui.add(Label::new("click me").sense(Sense::click())).clicked() {
	///     /* … */
	/// }
	/// # });
	/// ```
	pub fn sense(mut self, sense: Sense) -> Self {
		self.sense = Some(sense);
		self
	}
}

#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct Button {
	text: WidgetText,
	shortcut_text: WidgetText,
	wrap: Option<bool>,
	/// None means default for interact
	fill: Option<Color32>,
	stroke: Option<Stroke>,
	sense: Sense,
	small: bool,
	frame: Option<bool>,
	min_size: Vec2,
	rounding: Option<Rounding>,
	image: Option<Image>,
}

impl Button {
	pub fn new(text: impl Into<WidgetText>) -> Self {
		Self {
			text: text.into(),
			shortcut_text: Default::default(),
			wrap: None,
			fill: None,
			stroke: None,
			sense: Sense::click(),
			small: false,
			frame: None,
			min_size: Vec2::ZERO,
			rounding: None,
			image: None,
		}
	}

	/// Creates a button with an image to the left of the text. The size of the image as displayed is defined by the provided size.
	#[allow(clippy::needless_pass_by_value)]
	pub fn image_and_text(
		texture_id: TextureId,
		image_size: impl Into<Vec2>,
		text: impl Into<WidgetText>,
	) -> Self {
		Self {
			image: Some(Image::new(texture_id, image_size)),
			..Self::new(text)
		}
	}

	/// If `true`, the text will wrap to stay within the max width of the [`Ui`].
	///
	/// By default [`Self::wrap`] will be true in vertical layouts
	/// and horizontal layouts with wrapping,
	/// and false on non-wrapping horizontal layouts.
	///
	/// Note that any `\n` in the text will always produce a new line.
	#[inline]
	pub fn wrap(mut self, wrap: bool) -> Self {
		self.wrap = Some(wrap);
		self
	}

	/// Override background fill color. Note that this will override any on-hover effects.
	/// Calling this will also turn on the frame.
	pub fn fill(mut self, fill: impl Into<Color32>) -> Self {
		self.fill = Some(fill.into());
		self.frame = Some(true);
		self
	}

	/// Override button stroke. Note that this will override any on-hover effects.
	/// Calling this will also turn on the frame.
	pub fn stroke(mut self, stroke: impl Into<Stroke>) -> Self {
		self.stroke = Some(stroke.into());
		self.frame = Some(true);
		self
	}

	/// Make this a small button, suitable for embedding into text.
	pub fn small(mut self) -> Self {
		self.text = self.text.text_style(TextStyle::Body);
		self.small = true;
		self
	}

	/// Turn off the frame
	pub fn frame(mut self, frame: bool) -> Self {
		self.frame = Some(frame);
		self
	}

	/// By default, buttons senses clicks.
	/// Change this to a drag-button with `Sense::drag()`.
	pub fn sense(mut self, sense: Sense) -> Self {
		self.sense = sense;
		self
	}

	/// Set the minimum size of the button.
	pub fn min_size(mut self, min_size: Vec2) -> Self {
		self.min_size = min_size;
		self
	}

	/// Set the rounding of the button.
	pub fn rounding(mut self, rounding: impl Into<Rounding>) -> Self {
		self.rounding = Some(rounding.into());
		self
	}

	/// Show some text on the right side of the button, in weak color.
	///
	/// Designed for menu buttons, for setting a keyboard shortcut text (e.g. `Ctrl+S`).
	///
	/// The text can be created with [`Context::format_shortcut`].
	pub fn shortcut_text(mut self, shortcut_text: impl Into<WidgetText>) -> Self {
		self.shortcut_text = shortcut_text.into();
		self
	}
}

#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct Checkbox {
	checked: bool,
	text: WidgetText,
}

impl Checkbox {
	pub fn new(checked: bool, text: impl Into<WidgetText>) -> Self {
		Checkbox {
			checked,
			text: text.into(),
		}
	}

	pub fn without_text(checked: bool) -> Self {
		Self::new(checked, WidgetText::default())
	}
}

// ----------------------------------------------------------------------------

/// One out of several alternatives, either selected or not.
///
/// Usually you'd use [`Ui::radio_value`] or [`Ui::radio`] instead.
///
/// ```
/// # egui::__run_test_ui(|ui| {
/// #[derive(PartialEq)]
/// enum Enum { First, Second, Third }
/// let mut my_enum = Enum::First;
///
/// ui.radio_value(&mut my_enum, Enum::First, "First");
///
/// // is equivalent to:
///
/// if ui.add(egui::RadioButton::new(my_enum == Enum::First, "First")).clicked() {
///	 my_enum = Enum::First
/// }
/// # });
/// ```
#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct RadioButton {
	checked: bool,
	text: WidgetText,
}

impl RadioButton {
	pub fn new(checked: bool, text: impl Into<WidgetText>) -> Self {
		Self {
			checked,
			text: text.into(),
		}
	}
}

pub trait Create<W> {
	type LaidOutWidget;
	fn create(&mut self, widget: W) -> Self::LaidOutWidget;
}


impl Create<Label> for WidgetPlacer<'_> {
	type LaidOutWidget = LaidOutLabel;
	fn create(&mut self, label: Label) -> LaidOutLabel {
		let sense = label.sense.unwrap_or_else(|| {
			// We only want to focus labels if the screen reader is on.
			if self.context.memory(|mem| mem.options.screen_reader) {
				Sense::focusable_noninteractive()
			} else {
				Sense::hover()
			}
		});
		if let WidgetText::Galley(galley) = label.text {
			// If the user said "use this specific galley", then just use it:
			let (rect, response_rect) = self.allocate_exact_size(galley.size());
			let pos = match galley.job.halign {
				Align::LEFT => rect.left_top(),
				Align::Center => rect.center_top(),
				Align::RIGHT => rect.right_top(),
			};
			let text_galley = WidgetTextGalley {
				galley,
				galley_has_color: true,
			};
			return LaidOutLabel { pos, text_galley, response_rect, sense }
		}

		let valign = self.placer.layout().vertical_align();
		let mut text_job = label
			.text
			.into_text_job(self.style, FontSelection::Default, valign);

		let should_wrap = label.wrap.unwrap_or_else(|| self.wrap_text());
		let available_width = self.placer.available_size().x;

		if should_wrap
			&& self.placer.layout().main_dir() == Direction::LeftToRight
			&& self.placer.layout().main_wrap()
			&& available_width.is_finite()
		{
			// On a wrapping horizontal layout we want text to start after the previous widget,
			// then continue on the line below! This will take some extra work:

			let cursor = self.placer.cursor();
			let first_row_indentation = available_width - self.placer.available_rect_before_wrap().size().x;
			egui::egui_assert!(first_row_indentation.is_finite());

			text_job.job.wrap.max_width = available_width;
			text_job.job.first_row_min_height = cursor.height();
			text_job.job.halign = Align::Min;
			text_job.job.justify = false;
			if let Some(first_section) = text_job.job.sections.first_mut() {
				first_section.leading_space = first_row_indentation;
			}
			let text_galley = self.context.fonts(|f| text_job.into_galley(f));

			let pos = pos2(self.placer.max_rect().left(), self.placer.cursor().top());
			assert!(
				!text_galley.galley.rows.is_empty(),
				"Galleys are never empty"
			);
			// collect a response from many rows:
			let mut response_rect = text_galley.galley.rows[0]
				.rect
				.translate(Vec2::new(pos.x, pos.y));
			self.allocate_rect(response_rect);
			for row in text_galley.galley.rows.iter().skip(1) {
				let rect = row.rect.translate(Vec2::new(pos.x, pos.y));
				self.allocate_rect(rect);
				response_rect = response_rect.union(rect);
			}
			LaidOutLabel { pos, text_galley, response_rect, sense }
		} else {
			if should_wrap {
				text_job.job.wrap.max_width = available_width;
			} else {
				text_job.job.wrap.max_width = f32::INFINITY;
			};

			if self.placer.is_grid() {
				// TODO(emilk): remove special Grid hacks like these
				text_job.job.halign = Align::LEFT;
				text_job.job.justify = false;
			} else {
				text_job.job.halign = self.placer.layout().horizontal_placement();
				text_job.job.justify = self.placer.layout().horizontal_justify();
			};

			let text_galley = self.context.fonts(|f| text_job.into_galley(f));
			let (rect, response_rect) = self.allocate_exact_size(text_galley.size());
			let pos = match text_galley.galley.job.halign {
				Align::LEFT => rect.left_top(),
				Align::Center => rect.center_top(),
				Align::RIGHT => rect.right_top(),
			};
			LaidOutLabel { pos, text_galley, response_rect, sense }
		}
	}
}

impl Create<Button> for WidgetPlacer<'_> {
	type LaidOutWidget = LaidOutButton;
	fn create(&mut self, button: Button) -> LaidOutButton {
		let Button {
			text,
			shortcut_text,
			wrap,
			fill,
			stroke,
			sense,
			small,
			frame,
			min_size,
			rounding,
			image,
		} = button;

		let frame = frame.unwrap_or_else(|| self.style.visuals.button_frame);

		let mut button_padding = self.style.spacing.button_padding;
		if small {
			button_padding.y = 0.0;
		}

		let mut text_wrap_width = self.placer.available_size().x - 2.0 * button_padding.x;
		if let Some(image) = image {
			text_wrap_width -= image.size().x + self.style.spacing.icon_spacing;
		}
		if !shortcut_text.is_empty() {
			text_wrap_width -= 60.0; // Some space for the shortcut text (which we never wrap).
		}

		let text = self.into_galley(text, wrap, text_wrap_width, TextStyle::Button);
		let shortcut_text = (!shortcut_text.is_empty())
			.then(|| self.into_galley(shortcut_text, Some(false), f32::INFINITY, TextStyle::Button));

		let mut desired_size = text.size();
		if let Some(image) = image {
			desired_size.x += image.size().x + self.style.spacing.icon_spacing;
			desired_size.y = desired_size.y.max(image.size().y);
		}
		if let Some(shortcut_text) = &shortcut_text {
			desired_size.x += self.style.spacing.item_spacing.x + shortcut_text.size().x;
			desired_size.y = desired_size.y.max(shortcut_text.size().y);
		}
		desired_size += 2.0 * button_padding;
		if !small {
			desired_size.y = desired_size.y.at_least(self.style.spacing.interact_size.y);
		}
		desired_size = desired_size.at_least(min_size);

		let rect = self.allocate_space(desired_size);

		LaidOutButton { rect, frame, fill, stroke, rounding, image, button_padding, text, shortcut_text, sense }
	}
}

impl Create<Checkbox> for WidgetPlacer<'_> {
	type LaidOutWidget = LaidOutCheckbox;
	fn create(&mut self, checkbox: Checkbox) -> LaidOutCheckbox {
		let Checkbox { checked, text } = checkbox;

		let spacing = &self.style.spacing;
		let icon_width = spacing.icon_width;
		let icon_spacing = spacing.icon_spacing;

		let (text, mut desired_size) = if text.is_empty() {
			(None, Vec2::new(icon_width, 0.0))
		} else {
			let total_extra = Vec2::new(icon_width + icon_spacing, 0.0);

			let wrap_width = self.placer.available_size().x - total_extra.x;
			let text = self.into_galley(text, None, wrap_width, TextStyle::Button);

			let mut desired_size = total_extra + text.size();
			desired_size = desired_size.at_least(spacing.interact_size);

			(Some(text), desired_size)
		};

		desired_size = desired_size.at_least(Vec2::splat(spacing.interact_size.y));
		desired_size.y = desired_size.y.max(icon_width);
		let (rect, response_rect) = self.allocate_exact_size(desired_size);

		LaidOutCheckbox { rect, response_rect, checked, text, icon_width, icon_spacing }
	}
}

impl Create<RadioButton> for WidgetPlacer<'_> {
	type LaidOutWidget = LaidOutRadioButton;
	fn create(&mut self, radio: RadioButton) -> LaidOutRadioButton {
		let RadioButton { checked, text } = radio;

		let spacing = &self.style.spacing;
		let icon_width = spacing.icon_width;
		let icon_spacing = spacing.icon_spacing;

		let (text, mut desired_size) = if text.is_empty() {
			(None, Vec2::new(icon_width, 0.0))
		} else {
			let total_extra = Vec2::new(icon_width + icon_spacing, 0.0);

			let wrap_width = self.placer.available_size().x - total_extra.x;
			let text = self.into_galley(text, None, wrap_width, TextStyle::Button);

			let mut desired_size = total_extra + text.size();
			desired_size = desired_size.at_least(spacing.interact_size);

			(Some(text), desired_size)
		};

		desired_size = desired_size.at_least(Vec2::splat(spacing.interact_size.y));
		desired_size.y = desired_size.y.max(icon_width);
		let (rect, response_rect) = self.allocate_exact_size(desired_size);

		LaidOutRadioButton { rect, response_rect, checked, text, icon_width, icon_spacing }
	}
}

pub struct LaidOutLabel {
	pos: Pos2,
	response_rect: Rect,
	text_galley: WidgetTextGalley,
	sense: Sense,
}

impl LaidOutLabel {
	pub fn reposition(&mut self, y: f32) {
		let diff = Vec2::new(0., y - self.response_rect.top());
		self.pos += diff;
		self.response_rect = self.response_rect.translate(diff);
	}

	pub fn interact(&self, ui: &mut Ui) -> Response {
		let response = ui.interact(self.response_rect, ui.next_auto_id(), self.sense);
		ui.skip_ahead_auto_ids(1);
		response
	}
}

pub struct LaidOutButton {
	rect: Rect,
	frame: bool,
	fill: Option<Color32>,
	stroke: Option<Stroke>,
	rounding: Option<Rounding>,
	image: Option<Image>,
	button_padding: Vec2,
	text: WidgetTextGalley,
	shortcut_text: Option<WidgetTextGalley>,
	sense: Sense,
}

impl LaidOutButton {
	pub fn reposition(&mut self, y: f32) {
		let d = self.rect.height() / 2.0;
		self.rect.max.y = y + d;
		self.rect.min.y = y - d;
	}

	pub fn interact(&self, ui: &mut Ui) -> Response {
		let response = ui.interact(self.rect, ui.next_auto_id(), self.sense);
		ui.skip_ahead_auto_ids(1);
		response
	}
}

pub struct LaidOutCheckbox {
	rect: Rect,
	response_rect: Rect,
	checked: bool,
	text: Option<WidgetTextGalley>,
	icon_width: f32,
	icon_spacing: f32,
}

impl LaidOutCheckbox {
	pub fn reposition(&mut self, y: f32) {
		let d = self.rect.height() / 2.0;
		self.rect.max.y = y + d;
		self.rect.min.y = y - d;
		let d = self.response_rect.height() / 2.0;
		self.response_rect.max.y = y + d;
		self.response_rect.min.y = y - d;
	}

	pub fn interact(&self, ui: &mut Ui) -> Response {
		let response = ui.interact(self.response_rect, ui.next_auto_id(), Sense::click());
		ui.skip_ahead_auto_ids(1);
		response
	}
}

pub struct LaidOutRadioButton {
	rect: Rect,
	response_rect: Rect,
	checked: bool,
	text: Option<WidgetTextGalley>,
	icon_width: f32,
	icon_spacing: f32,
}

impl LaidOutRadioButton {
	pub fn reposition(&mut self, y: f32) {
		self.rect.max.y = y + self.rect.height();
		self.rect.min.y = y;
		self.response_rect.max.y = y + self.response_rect.height();
		self.response_rect.min.y = y;
	}

	pub fn interact(&self, ui: &mut Ui) -> Response {
		let response = ui.interact(self.response_rect, ui.next_auto_id(), Sense::click());
		ui.skip_ahead_auto_ids(1);
		response
	}
}

pub trait Paint<W> {
	fn paint(&mut self, lowidget: &W, response: &Response);
}

impl Paint<LaidOutLabel> for Ui {
	fn paint(&mut self, lolabel: &LaidOutLabel, response: &Response) {
		let LaidOutLabel { pos, text_galley, .. } = lolabel;
		if self.is_rect_visible(response.rect) {
			let response_color = self.style().interact(&response).text_color();

			let underline = if response.has_focus() || response.highlighted() {
				Stroke::new(1.0, response_color)
			} else {
				Stroke::NONE
			};

			let override_text_color = if text_galley.galley_has_color {
				None
			} else {
				Some(response_color)
			};

			self.painter().add(TextShape {
				pos: *pos,
				galley: text_galley.galley.clone(),
				override_text_color,
				underline,
				angle: 0.0,
			});
		}
	}
}

impl Paint<LaidOutButton> for Ui {
	fn paint(&mut self, lobutton: &LaidOutButton, response: &Response) {
		let &LaidOutButton {
			rect,
			frame,
			fill,
			stroke,
			rounding,
			image,
			button_padding,
			ref text,
			ref shortcut_text,
			..
		} = lobutton;
		if self.is_rect_visible(rect) {
			let visuals = self.style().interact(response);

			if frame {
				let fill = fill.unwrap_or(visuals.weak_bg_fill);
				let stroke = stroke.unwrap_or(visuals.bg_stroke);
				let rounding = rounding.unwrap_or(visuals.rounding);
				self.painter()
					.rect(rect.expand(visuals.expansion), rounding, fill, stroke);
			}

			let text_pos = if let Some(image) = image {
				let icon_spacing = self.spacing().icon_spacing;
				pos2(
					rect.min.x + button_padding.x + image.size().x + icon_spacing,
					rect.center().y - 0.5 * text.size().y,
				)
			} else {
				self.layout()
					.align_size_within_rect(text.size(), rect.shrink2(button_padding))
					.min
			};
			text.clone().paint_with_visuals(self.painter(), text_pos, visuals);

			if let Some(shortcut_text) = shortcut_text {
				let shortcut_text_pos = pos2(
					rect.max.x - button_padding.x - shortcut_text.size().x,
					rect.center().y - 0.5 * shortcut_text.size().y,
				);
				shortcut_text.clone().paint_with_fallback_color(
					self.painter(),
					shortcut_text_pos,
					self.visuals().weak_text_color(),
				);
			}

			if let Some(image) = image {
				let image_rect = Rect::from_min_size(
					pos2(
						rect.min.x + button_padding.x,
						rect.center().y - 0.5 - (image.size().y / 2.0),
					),
					image.size(),
				);
				image.paint_at(self, image_rect);
			}
		}
	}
}

impl Paint<LaidOutCheckbox> for Ui {
	fn paint(&mut self, locheckbox: &LaidOutCheckbox, response: &Response) {
		let &LaidOutCheckbox { rect, checked, ref text, icon_width, icon_spacing, .. } = locheckbox;
		if self.is_rect_visible(rect) {
			// let visuals = self.style().interact_selectable(&response, *checked); // too colorful
			let visuals = self.style().interact(&response);
			let (small_icon_rect, big_icon_rect) = self.spacing().icon_rectangles(rect);
			self.painter().add(epaint::RectShape {
				rect: big_icon_rect.expand(visuals.expansion),
				rounding: visuals.rounding,
				fill: visuals.bg_fill,
				stroke: visuals.bg_stroke,
			});

			if checked {
				// Check mark:
				self.painter().add(Shape::line(
					vec![
						pos2(small_icon_rect.left(), small_icon_rect.center().y),
						pos2(small_icon_rect.center().x, small_icon_rect.bottom()),
						pos2(small_icon_rect.right(), small_icon_rect.top()),
					],
					visuals.fg_stroke,
				));
			}
			if let Some(text) = text {
				let text_pos = pos2(
					rect.min.x + icon_width + icon_spacing,
					rect.center().y - 0.5 * text.size().y,
				);
				text.clone().paint_with_visuals(self.painter(), text_pos, visuals);
			}
		}
	}
}

impl Paint<LaidOutRadioButton> for Ui {
	fn paint(&mut self, lorbutton: &LaidOutRadioButton, response: &Response) {
		let &LaidOutRadioButton { rect, checked, ref text, icon_width, icon_spacing, .. } = lorbutton;
		if self.is_rect_visible(rect) {
			// let visuals = self.style().interact_selectable(&response, checked); // too colorful
			let visuals = self.style().interact(&response);

			let (small_icon_rect, big_icon_rect) = self.spacing().icon_rectangles(rect);

			let painter = self.painter();

			painter.add(epaint::CircleShape {
				center: big_icon_rect.center(),
				radius: big_icon_rect.width() / 2.0 + visuals.expansion,
				fill: visuals.bg_fill,
				stroke: visuals.bg_stroke,
			});

			if checked {
				painter.add(epaint::CircleShape {
					center: small_icon_rect.center(),
					radius: small_icon_rect.width() / 3.0,
					fill: visuals.fg_stroke.color, // Intentional to use stroke and not fill
					// fill: self.visuals().selection.stroke.color, // too much color
					stroke: Default::default(),
				});
			}

			if let Some(text) = text {
				let text_pos = pos2(
					rect.min.x + icon_width + icon_spacing,
					rect.center().y - 0.5 * text.size().y,
				);
				text.clone().paint_with_visuals(self.painter(), text_pos, visuals);
			}
		}
	}
}
