#include <pebble.h>

// Command values (must match pkjs)
#define CMD_REQUEST_DEPARTURES 0
#define CMD_DEPARTURE_DATA 1
#define CMD_STATUS 2
#define CMD_REQUEST_NEARBY 3
#define CMD_NEARBY_DATA 4

#define MAX_STOPS 5
#define MAX_DEPARTURES 3
#define BUF_LEN 32
#define ACCENT_COLOR GColorImperialPurple

// --- Stops screen state ---
static Window *s_stops_window;
static TextLayer *s_stops_status_layer;
static SimpleMenuLayer *s_stops_menu_layer;

static char s_stop_ids[MAX_STOPS][BUF_LEN];
static char s_stop_titles[MAX_STOPS][BUF_LEN];
static char s_stop_subtitles[MAX_STOPS][BUF_LEN];
static int s_stop_count = 0;

static SimpleMenuItem s_stop_items[MAX_STOPS];
static SimpleMenuSection s_stop_sections[] = {
    {.title = "Nearby Stops", .items = NULL, .num_items = 0},
};

// --- Departures screen state ---
static Window *s_dep_window;
static TextLayer *s_dep_title_layer;
static TextLayer *s_dep_status_layer;
static SimpleMenuLayer *s_dep_menu_layer;

static char s_dep_stop_name[BUF_LEN] = "";
static char s_dep_titles[MAX_DEPARTURES][BUF_LEN];
static char s_dep_subtitles[MAX_DEPARTURES][BUF_LEN];
static int s_dep_count = 0;

static SimpleMenuItem s_dep_items[MAX_DEPARTURES];
static SimpleMenuSection s_dep_sections[] = {
    {.title = NULL, .items = NULL, .num_items = 0},
};

// --- Shared status text ---
static char s_status_text[BUF_LEN] = "Connecting";
static GRect s_stops_bounds;
static GRect s_dep_bounds;

// --- Helpers ---

static void configure_text_layer(TextLayer *layer, GColor color,
                                 GTextAlignment alignment,
                                 const char *font_key)
{
  text_layer_set_background_color(layer, GColorClear);
  text_layer_set_text_color(layer, color);
  text_layer_set_text_alignment(layer, alignment);
  text_layer_set_font(layer, fonts_get_system_font(font_key));
}

// --- Departures window ---

static void refresh_dep_menu(void)
{
  for (int i = 0; i < s_dep_count; i++)
  {
    s_dep_items[i].title = s_dep_titles[i];
    s_dep_items[i].subtitle = s_dep_subtitles[i];
  }
  s_dep_sections[0].items = s_dep_items;
  s_dep_sections[0].num_items = s_dep_count;

  if (s_dep_menu_layer)
  {
    menu_layer_reload_data(simple_menu_layer_get_menu_layer(s_dep_menu_layer));
  }
}

static void dep_window_load(Window *window)
{
  Layer *root = window_get_root_layer(window);
  GRect bounds = layer_get_bounds(root);
  s_dep_bounds = bounds;

  window_set_background_color(window, ACCENT_COLOR);

  s_dep_title_layer = text_layer_create(GRect(6, 8, bounds.size.w - 12, 52));
  configure_text_layer(s_dep_title_layer, GColorWhite, GTextAlignmentCenter,
                       FONT_KEY_GOTHIC_24_BOLD);
  text_layer_set_text(s_dep_title_layer, s_dep_stop_name);

  s_dep_menu_layer = simple_menu_layer_create(
      GRect(0, 64, bounds.size.w, bounds.size.h - 88), window, s_dep_sections,
      ARRAY_LENGTH(s_dep_sections), NULL);

  s_dep_status_layer =
      text_layer_create(GRect(6, bounds.size.h - 24, bounds.size.w - 12, 18));
  configure_text_layer(s_dep_status_layer, GColorWhite, GTextAlignmentCenter,
                       FONT_KEY_GOTHIC_18);
  text_layer_set_text(s_dep_status_layer, s_status_text);

  layer_add_child(root, text_layer_get_layer(s_dep_title_layer));
  layer_add_child(root, simple_menu_layer_get_layer(s_dep_menu_layer));
  layer_add_child(root, text_layer_get_layer(s_dep_status_layer));
}

static void dep_window_unload(Window *window)
{
  (void)window;
  text_layer_destroy(s_dep_title_layer);
  simple_menu_layer_destroy(s_dep_menu_layer);
  text_layer_destroy(s_dep_status_layer);
  s_dep_title_layer = NULL;
  s_dep_menu_layer = NULL;
  s_dep_status_layer = NULL;
}

// --- Stops window ---

static void stop_select_callback(int index, void *context)
{
  (void)context;
  if (index < 0 || index >= s_stop_count)
    return;

  // Reset departure state
  s_dep_count = 0;
  strncpy(s_dep_stop_name, s_stop_titles[index], BUF_LEN - 1);
  strncpy(s_status_text, "Loading...", BUF_LEN - 1);

  // Push departures window
  s_dep_window = window_create();
  window_set_window_handlers(s_dep_window, (WindowHandlers){
                                               .load = dep_window_load,
                                               .unload = dep_window_unload,
                                           });
  window_stack_push(s_dep_window, true);

  // Request departures from JS
  DictionaryIterator *iter;
  AppMessageResult result = app_message_outbox_begin(&iter);
  if (result != APP_MSG_OK)
    return;

  dict_write_int32(iter, MESSAGE_KEY_Command, CMD_REQUEST_DEPARTURES);
  dict_write_cstring(iter, MESSAGE_KEY_StopId, s_stop_ids[index]);
  app_message_outbox_send();
}

static void refresh_stops_menu(void)
{
  for (int i = 0; i < s_stop_count; i++)
  {
    s_stop_items[i].title = s_stop_titles[i];
    s_stop_items[i].subtitle = s_stop_subtitles[i];
    s_stop_items[i].callback = stop_select_callback;
  }
  s_stop_sections[0].items = s_stop_items;
  s_stop_sections[0].num_items = s_stop_count;

  if (s_stops_menu_layer)
  {
    menu_layer_reload_data(simple_menu_layer_get_menu_layer(s_stops_menu_layer));
  }
}

static void stops_window_load(Window *window)
{
  Layer *root = window_get_root_layer(window);
  GRect bounds = layer_get_bounds(root);
  s_stops_bounds = bounds;

  window_set_background_color(window, ACCENT_COLOR);

  s_stops_menu_layer = simple_menu_layer_create(
      GRect(0, 0, bounds.size.w, bounds.size.h - 24), window, s_stop_sections,
      ARRAY_LENGTH(s_stop_sections), NULL);

  s_stops_status_layer =
      text_layer_create(GRect(6, bounds.size.h - 24, bounds.size.w - 12, 18));
  configure_text_layer(s_stops_status_layer, GColorWhite, GTextAlignmentCenter,
                       FONT_KEY_GOTHIC_18);
  text_layer_set_text(s_stops_status_layer, s_status_text);

  layer_add_child(root, simple_menu_layer_get_layer(s_stops_menu_layer));
  layer_add_child(root, text_layer_get_layer(s_stops_status_layer));
}

static void stops_window_unload(Window *window)
{
  (void)window;
  simple_menu_layer_destroy(s_stops_menu_layer);
  text_layer_destroy(s_stops_status_layer);
  s_stops_menu_layer = NULL;
  s_stops_status_layer = NULL;
}

// --- AppMessage handlers ---

static void update_status(const char *text)
{
  bool hide = (strcmp(text, "OK") == 0);
  strncpy(s_status_text, text, BUF_LEN - 1);

  if (s_stops_status_layer)
  {
    layer_set_hidden(text_layer_get_layer(s_stops_status_layer), hide);
    text_layer_set_text(s_stops_status_layer, s_status_text);
  }
  if (s_stops_menu_layer)
  {
    Layer *ml = simple_menu_layer_get_layer(s_stops_menu_layer);
    GRect frame = layer_get_frame(ml);
    frame.size.h = hide ? s_stops_bounds.size.h : s_stops_bounds.size.h - 24;
    layer_set_frame(ml, frame);
  }

  if (s_dep_status_layer)
  {
    layer_set_hidden(text_layer_get_layer(s_dep_status_layer), hide);
    text_layer_set_text(s_dep_status_layer, s_status_text);
  }
  if (s_dep_menu_layer)
  {
    Layer *ml = simple_menu_layer_get_layer(s_dep_menu_layer);
    GRect frame = layer_get_frame(ml);
    frame.size.h =
        hide ? (s_dep_bounds.size.h - 64) : (s_dep_bounds.size.h - 88);
    layer_set_frame(ml, frame);
  }
}

static void inbox_received_handler(DictionaryIterator *iter, void *context)
{
  Tuple *cmd_tuple = dict_find(iter, MESSAGE_KEY_Command);
  if (!cmd_tuple)
    return;

  int cmd = cmd_tuple->value->int32;

  if (cmd == CMD_NEARBY_DATA)
  {
    Tuple *count_tuple = dict_find(iter, MESSAGE_KEY_Count);
    if (count_tuple)
    {
      s_stop_count = count_tuple->value->int32;
      if (s_stop_count > MAX_STOPS)
        s_stop_count = MAX_STOPS;
    }

    Tuple *index_tuple = dict_find(iter, MESSAGE_KEY_Index);
    if (index_tuple)
    {
      int idx = index_tuple->value->int32;
      if (idx >= 0 && idx < MAX_STOPS)
      {
        Tuple *id = dict_find(iter, MESSAGE_KEY_StopId);
        Tuple *name = dict_find(iter, MESSAGE_KEY_StopName);
        Tuple *dist = dict_find(iter, MESSAGE_KEY_Distance);

        if (id)
        {
          strncpy(s_stop_ids[idx], id->value->cstring, BUF_LEN - 1);
        }
        if (name)
        {
          strncpy(s_stop_titles[idx], name->value->cstring, BUF_LEN - 1);
        }
        if (dist)
        {
          snprintf(s_stop_subtitles[idx], BUF_LEN, "%dm away",
                   (int)dist->value->int32);
        }

        refresh_stops_menu();
      }
    }
  }
  else if (cmd == CMD_DEPARTURE_DATA)
  {
    Tuple *name_tuple = dict_find(iter, MESSAGE_KEY_StopName);
    if (name_tuple)
    {
      strncpy(s_dep_stop_name, name_tuple->value->cstring, BUF_LEN - 1);
      if (s_dep_title_layer)
        text_layer_set_text(s_dep_title_layer, s_dep_stop_name);
    }

    Tuple *count_tuple = dict_find(iter, MESSAGE_KEY_Count);
    if (count_tuple)
    {
      s_dep_count = count_tuple->value->int32;
      if (s_dep_count > MAX_DEPARTURES)
        s_dep_count = MAX_DEPARTURES;
    }

    Tuple *index_tuple = dict_find(iter, MESSAGE_KEY_Index);
    if (index_tuple)
    {
      int idx = index_tuple->value->int32;
      if (idx >= 0 && idx < MAX_DEPARTURES)
      {
        Tuple *route = dict_find(iter, MESSAGE_KEY_Route);
        Tuple *minutes = dict_find(iter, MESSAGE_KEY_Minutes);
        Tuple *headsign = dict_find(iter, MESSAGE_KEY_Headsign);

        if (route && minutes)
        {
          snprintf(s_dep_titles[idx], BUF_LEN, "%s %dm", route->value->cstring,
                   (int)minutes->value->int32);
        }
        if (headsign)
        {
          strncpy(s_dep_subtitles[idx], headsign->value->cstring, BUF_LEN - 1);
        }

        refresh_dep_menu();
      }
    }
  }
  else if (cmd == CMD_STATUS)
  {
    Tuple *status_tuple = dict_find(iter, MESSAGE_KEY_Status);
    if (status_tuple)
    {
      update_status(status_tuple->value->cstring);
    }
  }
}

static void inbox_dropped_handler(AppMessageResult reason, void *context)
{
  APP_LOG(APP_LOG_LEVEL_ERROR, "Message dropped: %d", reason);
  update_status("Msg dropped");
}

static void outbox_failed_handler(DictionaryIterator *iter,
                                  AppMessageResult reason, void *context)
{
  APP_LOG(APP_LOG_LEVEL_ERROR, "Outbox send failed: %d", reason);
}

// --- Lifecycle ---

static void init(void)
{
  app_message_register_inbox_received(inbox_received_handler);
  app_message_register_inbox_dropped(inbox_dropped_handler);
  app_message_register_outbox_failed(outbox_failed_handler);
  app_message_open(256, 128);

  s_stops_window = window_create();
  window_set_window_handlers(s_stops_window, (WindowHandlers){
                                                 .load = stops_window_load,
                                                 .unload = stops_window_unload,
                                             });
  window_stack_push(s_stops_window, true);
}

static void deinit(void) { window_destroy(s_stops_window); }

int main(void)
{
  init();
  app_event_loop();
  deinit();
}
