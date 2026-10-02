#include "my_application.h"

#include <flutter_linux/flutter_linux.h>
#include <pango/pangocairo.h>

#include <cstring>
#include <set>
#include <string>
#ifdef GDK_WINDOWING_X11
#include <gdk/gdkx.h>
#endif

#include "flutter/generated_plugin_registrant.h"

struct _MyApplication {
  GtkApplication parent_instance;
  char** dart_entrypoint_arguments;
  FlMethodChannel* file_drop_channel;
  FlMethodChannel* system_fonts_channel;
  FlMethodChannel* window_channel;
  gboolean file_drop_enabled;
};

G_DEFINE_TYPE(MyApplication, my_application, GTK_TYPE_APPLICATION)

static GtkTargetEntry file_drop_targets[] = {
    {const_cast<gchar*>("text/uri-list"), 0, 0},
    {const_cast<gchar*>("x-special/gnome-icon-list"), 0, 0},
    {const_cast<gchar*>("text/plain;charset=utf-8"), 0, 0},
    {const_cast<gchar*>("text/plain"), 0, 0},
    {const_cast<gchar*>("UTF8_STRING"), 0, 0},
    {const_cast<gchar*>("STRING"), 0, 0},
};

static constexpr char kFileDropChannelName[] = "com.korvect.nauterm/file_drop";
static constexpr char kFileDropWidgetDataKey[] = "nauterm-file-drop-widget";
static constexpr char kSystemFontsChannelName[] =
    "com.korvect.nauterm/system_fonts";

static FlValue* system_font_families(gboolean monospace_only) {
  PangoFontMap* font_map = pango_cairo_font_map_get_default();
  if (font_map == nullptr) {
    return fl_value_new_list();
  }

  PangoFontFamily** families = nullptr;
  int family_count = 0;
  pango_font_map_list_families(font_map, &families, &family_count);

  std::set<std::string> names;
  for (int index = 0; index < family_count; index++) {
    PangoFontFamily* family = families[index];
    const gchar* name = pango_font_family_get_name(family);
    if (name == nullptr || name[0] == '\0') {
      continue;
    }
    if (monospace_only && !pango_font_family_is_monospace(family)) {
      continue;
    }
    names.emplace(name);
  }
  g_free(families);

  FlValue* result = fl_value_new_list();
  for (const std::string& name : names) {
    fl_value_append_take(result, fl_value_new_string(name.c_str()));
  }
  return result;
}

static void system_fonts_method_call_cb(FlMethodChannel* channel,
                                        FlMethodCall* method_call,
                                        gpointer user_data) {
  const gchar* method = fl_method_call_get_name(method_call);
  g_autoptr(GError) error = nullptr;

  if (std::strcmp(method, "listMonospaceFamilies") == 0) {
    g_autoptr(FlValue) result = system_font_families(TRUE);
    fl_method_call_respond_success(method_call, result, &error);
  } else if (std::strcmp(method, "listFontFamilies") == 0) {
    g_autoptr(FlValue) result = system_font_families(FALSE);
    fl_method_call_respond_success(method_call, result, &error);
  } else {
    fl_method_call_respond_not_implemented(method_call, &error);
  }

  if (error != nullptr) {
    g_warning("Failed to respond to system fonts method call: %s",
              error->message);
  }
}

static gboolean file_drop_enabled_from_args(FlValue* args) {
  if (args == nullptr || fl_value_get_type(args) != FL_VALUE_TYPE_MAP) {
    return FALSE;
  }

  FlValue* enabled = fl_value_lookup_string(args, "enabled");
  if (enabled == nullptr || fl_value_get_type(enabled) != FL_VALUE_TYPE_BOOL) {
    return FALSE;
  }

  return fl_value_get_bool(enabled) ? TRUE : FALSE;
}

static gboolean file_drop_drag_motion_cb(GtkWidget* widget,
                                         GdkDragContext* context,
                                         gint x,
                                         gint y,
                                         guint time,
                                         gpointer user_data);
static gboolean file_drop_drag_drop_cb(GtkWidget* widget,
                                       GdkDragContext* context,
                                       gint x,
                                       gint y,
                                       guint time,
                                       gpointer user_data);
static void file_drop_drag_leave_cb(GtkWidget* widget,
                                    GdkDragContext* context,
                                    guint time,
                                    gpointer user_data);
static void file_drop_drag_data_received_cb(GtkWidget* widget,
                                            GdkDragContext* context,
                                            gint x,
                                            gint y,
                                            GtkSelectionData* data,
                                            guint info,
                                            guint time,
                                            gpointer user_data);

struct FileDropSyncContext {
  MyApplication* self;
  gboolean enabled;
};

static void sync_file_drop_target(GtkWidget* target,
                                  MyApplication* self,
                                  gboolean enabled) {
  if (target == nullptr) {
    return;
  }

  if (g_object_get_data(G_OBJECT(target), kFileDropWidgetDataKey) == nullptr) {
    g_signal_connect(target, "drag-motion",
                     G_CALLBACK(file_drop_drag_motion_cb), self);
    g_signal_connect(target, "drag-drop", G_CALLBACK(file_drop_drag_drop_cb),
                     self);
    g_signal_connect(target, "drag-leave", G_CALLBACK(file_drop_drag_leave_cb),
                     self);
    g_signal_connect(target, "drag-data-received",
                     G_CALLBACK(file_drop_drag_data_received_cb), self);
    g_object_set_data(G_OBJECT(target), kFileDropWidgetDataKey, self);
  }

  if (enabled) {
    gtk_drag_dest_set(target, static_cast<GtkDestDefaults>(0),
                      file_drop_targets, G_N_ELEMENTS(file_drop_targets),
                      GDK_ACTION_COPY);
    gtk_drag_dest_set_track_motion(target, TRUE);
  } else {
    gtk_drag_dest_unset(target);
  }
}

static void sync_file_drop_widget_tree(GtkWidget* widget, gpointer user_data) {
  FileDropSyncContext* context =
      static_cast<FileDropSyncContext*>(user_data);
  sync_file_drop_target(widget, context->self, context->enabled);

  if (GTK_IS_CONTAINER(widget)) {
    gtk_container_foreach(GTK_CONTAINER(widget), sync_file_drop_widget_tree,
                          context);
  }
}

static void sync_file_drop_targets(MyApplication* self) {
  FileDropSyncContext context = {self, self->file_drop_enabled};
  GList* toplevels = gtk_window_list_toplevels();
  for (GList* item = toplevels; item != nullptr; item = item->next) {
    if (!GTK_IS_WIDGET(item->data)) {
      continue;
    }
    sync_file_drop_widget_tree(GTK_WIDGET(item->data), &context);
  }
  g_list_free(toplevels);
}

static gboolean file_drop_atom_matches(GdkAtom atom, const gchar* target) {
  g_autofree gchar* name = gdk_atom_name(atom);
  return name != nullptr && g_ascii_strcasecmp(name, target) == 0;
}

static GdkAtom file_drop_target_from_context(GdkDragContext* context) {
  GdkAtom fallback = GDK_NONE;
  for (GList* item = gdk_drag_context_list_targets(context); item != nullptr;
       item = item->next) {
    GdkAtom target = static_cast<GdkAtom>(item->data);
    if (file_drop_atom_matches(target, "text/uri-list")) {
      return target;
    }
    if (fallback == GDK_NONE &&
        (file_drop_atom_matches(target, "text/plain;charset=utf-8") ||
         file_drop_atom_matches(target, "x-special/gnome-icon-list") ||
         file_drop_atom_matches(target, "text/plain") ||
         file_drop_atom_matches(target, "UTF8_STRING") ||
         file_drop_atom_matches(target, "STRING"))) {
      fallback = target;
    }
  }
  return fallback;
}

static gboolean file_drop_append_uri_path(FlValue* paths, const gchar* uri) {
  if (uri == nullptr || uri[0] == '\0') {
    return FALSE;
  }

  g_autofree gchar* path = g_filename_from_uri(uri, nullptr, nullptr);
  if (path == nullptr || path[0] == '\0') {
    return FALSE;
  }

  fl_value_append_take(paths, fl_value_new_string(path));
  return TRUE;
}

static void file_drop_append_text_paths(FlValue* paths, const gchar* text) {
  if (text == nullptr || text[0] == '\0') {
    return;
  }

  g_auto(GStrv) lines = g_strsplit_set(text, "\r\n", -1);
  for (gchar** line = lines; line != nullptr && *line != nullptr; line++) {
    gchar* item = g_strstrip(*line);
    if (item[0] == '\0') {
      continue;
    }

    if (g_str_has_prefix(item, "file://")) {
      file_drop_append_uri_path(paths, item);
    } else if (g_path_is_absolute(item)) {
      fl_value_append_take(paths, fl_value_new_string(item));
    }
  }
}

static void file_drop_add_widget_coordinates(FlValue* args,
                                             GtkWidget* widget,
                                             gint x,
                                             gint y) {
  gint widget_x = x;
  gint widget_y = y;
  GtkWidget* toplevel = gtk_widget_get_toplevel(widget);
  if (GTK_IS_WIDGET(toplevel)) {
    gtk_widget_translate_coordinates(widget, toplevel, x, y, &widget_x,
                                     &widget_y);
  }
  fl_value_set_string_take(args, "x", fl_value_new_float(widget_x));
  fl_value_set_string_take(args, "y", fl_value_new_float(widget_y));
}

static FlValue* file_drop_paths_from_selection(GtkSelectionData* data) {
  FlValue* paths = fl_value_new_list();

  g_auto(GStrv) uris = gtk_selection_data_get_uris(data);
  if (uris != nullptr) {
    for (gchar** uri = uris; *uri != nullptr; uri++) {
      file_drop_append_uri_path(paths, *uri);
    }
  }

  if (fl_value_get_length(paths) == 0) {
    g_autofree gchar* text =
        reinterpret_cast<gchar*>(gtk_selection_data_get_text(data));
    file_drop_append_text_paths(paths, text);
  }

  return paths;
}

static void file_drop_method_call_cb(FlMethodChannel* channel,
                                     FlMethodCall* method_call,
                                     gpointer user_data) {
  MyApplication* self = MY_APPLICATION(user_data);
  const gchar* method = fl_method_call_get_name(method_call);
  g_autoptr(GError) error = nullptr;

  if (std::strcmp(method, "setEnabled") == 0) {
    self->file_drop_enabled = file_drop_enabled_from_args(
        fl_method_call_get_args(method_call));
    sync_file_drop_targets(self);
    fl_method_call_respond_success(method_call, nullptr, &error);
  } else {
    fl_method_call_respond_not_implemented(method_call, &error);
  }

  if (error != nullptr) {
    g_warning("Failed to respond to file drop method call: %s",
              error->message);
  }
}

static gboolean file_drop_drag_motion_cb(GtkWidget* widget,
                                         GdkDragContext* context,
                                         gint x,
                                         gint y,
                                         guint time,
                                         gpointer user_data) {
  MyApplication* self = MY_APPLICATION(user_data);
  if (!self->file_drop_enabled) {
    return FALSE;
  }

  if (file_drop_target_from_context(context) == GDK_NONE) {
    gdk_drag_status(context, static_cast<GdkDragAction>(0), time);
    return TRUE;
  }

  if (self->file_drop_channel != nullptr) {
    g_autoptr(FlValue) args = fl_value_new_map();
    file_drop_add_widget_coordinates(args, widget, x, y);
    fl_method_channel_invoke_method(self->file_drop_channel, "filesDragging",
                                    args, nullptr, nullptr, nullptr);
  }
  gdk_drag_status(context, GDK_ACTION_COPY, time);
  return TRUE;
}

static void file_drop_drag_leave_cb(GtkWidget* widget,
                                    GdkDragContext* context,
                                    guint time,
                                    gpointer user_data) {
  MyApplication* self = MY_APPLICATION(user_data);
  if (!self->file_drop_enabled || self->file_drop_channel == nullptr) {
    return;
  }

  g_autoptr(FlValue) args = fl_value_new_map();
  fl_method_channel_invoke_method(self->file_drop_channel, "filesExited", args,
                                  nullptr, nullptr, nullptr);
}

static gboolean file_drop_drag_drop_cb(GtkWidget* widget,
                                       GdkDragContext* context,
                                       gint x,
                                       gint y,
                                       guint time,
                                       gpointer user_data) {
  MyApplication* self = MY_APPLICATION(user_data);
  if (!self->file_drop_enabled) {
    return FALSE;
  }

  GdkAtom target = file_drop_target_from_context(context);
  if (target == GDK_NONE) {
    gtk_drag_finish(context, FALSE, FALSE, time);
    return TRUE;
  }

  gtk_drag_get_data(widget, context, target, time);
  return TRUE;
}

static void file_drop_drag_data_received_cb(GtkWidget* widget,
                                            GdkDragContext* context,
                                            gint x,
                                            gint y,
                                            GtkSelectionData* data,
                                            guint info,
                                            guint time,
                                            gpointer user_data) {
  MyApplication* self = MY_APPLICATION(user_data);
  if (!self->file_drop_enabled || self->file_drop_channel == nullptr) {
    gtk_drag_finish(context, FALSE, FALSE, time);
    return;
  }

  g_autoptr(FlValue) paths = file_drop_paths_from_selection(data);
  if (fl_value_get_length(paths) == 0) {
    gtk_drag_finish(context, FALSE, FALSE, time);
    return;
  }

  g_autoptr(FlValue) args = fl_value_new_map();
  fl_value_set_string_take(args, "paths", fl_value_ref(paths));
  file_drop_add_widget_coordinates(args, widget, x, y);
  fl_method_channel_invoke_method(self->file_drop_channel, "filesDropped",
                                  args, nullptr, nullptr, nullptr);
  gtk_drag_finish(context, TRUE, FALSE, time);
}

static void setup_file_drop_channel(MyApplication* self, FlView* view) {
  FlEngine* engine = fl_view_get_engine(view);
  FlBinaryMessenger* messenger = fl_engine_get_binary_messenger(engine);
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  self->file_drop_channel = fl_method_channel_new(
      messenger, kFileDropChannelName, FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(
      self->file_drop_channel, file_drop_method_call_cb, self, nullptr);

  sync_file_drop_targets(self);
}

static void setup_system_fonts_channel(MyApplication* self, FlView* view) {
  FlEngine* engine = fl_view_get_engine(view);
  FlBinaryMessenger* messenger = fl_engine_get_binary_messenger(engine);
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  self->system_fonts_channel = fl_method_channel_new(
      messenger, kSystemFontsChannelName, FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(
      self->system_fonts_channel, system_fonts_method_call_cb, self, nullptr);
}

static void window_method_call_cb(FlMethodChannel* channel,
                                 FlMethodCall* method_call,
                                 gpointer user_data) {
  if (std::strcmp(fl_method_call_get_name(method_call), "startWindowDrag") != 0) {
    fl_method_call_respond_not_implemented(method_call, nullptr);
    return;
  }
  FlValue* args = fl_method_call_get_args(method_call);
  FlValue* window_arg = nullptr;
  FlValue* view_arg = nullptr;
  if (args != nullptr && fl_value_get_type(args) == FL_VALUE_TYPE_MAP) {
    window_arg = fl_value_lookup_string(args, "window");
    view_arg = fl_value_lookup_string(args, "view");
  }
  if (window_arg == nullptr || view_arg == nullptr ||
      fl_value_get_type(window_arg) != FL_VALUE_TYPE_INT ||
      fl_value_get_type(view_arg) != FL_VALUE_TYPE_INT) {
    fl_method_call_respond_error(
        method_call, "invalid-arguments",
        "Window and view handles are required", nullptr, nullptr);
    return;
  }
  GtkWidget* window = reinterpret_cast<GtkWidget*>(fl_value_get_int(window_arg));
  GtkWidget* view = reinterpret_cast<GtkWidget*>(fl_value_get_int(view_arg));
  GdkWindow* surface = gtk_widget_get_window(window);
  GdkSeat* seat = surface == nullptr
                      ? nullptr
                      : gdk_display_get_default_seat(
                            gdk_window_get_display(surface));
  GdkDevice* pointer = seat == nullptr ? nullptr : gdk_seat_get_pointer(seat);
  if (pointer != nullptr) {
    const guint32 timestamp = gtk_get_current_event_time();
    gint root_x = 0, root_y = 0;
    gdk_device_get_position(pointer, nullptr, &root_x, &root_y);
    gdk_window_begin_move_drag_for_device(
        surface, pointer, GDK_BUTTON_PRIMARY, root_x, root_y, timestamp);
    // The compositor consumes the real button release during a move/snap.
    // Restore the synthetic release from the former vendored cnativeapi fix,
    // otherwise its pointer manager drops the next button press as a duplicate.
    GList* children = gtk_container_get_children(GTK_CONTAINER(view));
    for (GList* item = children; item != nullptr; item = item->next) {
      GtkWidget* event_box = GTK_WIDGET(item->data);
      if (!GTK_IS_EVENT_BOX(event_box)) {
        continue;
      }
      GdkWindow* event_window = gtk_widget_get_window(event_box);
      if (event_window == nullptr) {
        continue;
      }
      GdkEvent* release = gdk_event_new(GDK_BUTTON_RELEASE);
      release->button.window = GDK_WINDOW(g_object_ref(event_window));
      release->button.send_event = TRUE;
      release->button.time = timestamp;
      release->button.button = GDK_BUTTON_PRIMARY;
      gint x = 0, y = 0;
      GdkModifierType state = static_cast<GdkModifierType>(0);
      gdk_window_get_device_position(event_window, pointer, &x, &y, &state);
      release->button.x = x;
      release->button.y = y;
      release->button.x_root = root_x;
      release->button.y_root = root_y;
      release->button.state = static_cast<GdkModifierType>(0);
      gdk_event_set_device(release, pointer);
      gdk_event_set_source_device(release, pointer);
      gtk_widget_event(event_box, release);
      gdk_event_free(release);
    }
    g_list_free(children);
  }
  fl_method_call_respond_success(method_call, nullptr, nullptr);
}

static void setup_window_channel(MyApplication* self, FlView* view) {
  FlBinaryMessenger* messenger =
      fl_engine_get_binary_messenger(fl_view_get_engine(view));
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  self->window_channel = fl_method_channel_new(
      messenger, "com.korvect.nauterm/window", FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(
      self->window_channel, window_method_call_cb, self, nullptr);
}

// Called when first Flutter frame received.
static void first_frame_cb(MyApplication* self, FlView* view) {
  gtk_widget_show(gtk_widget_get_toplevel(GTK_WIDGET(view)));
}

static void set_nauterm_window_icon(GtkWindow* window) {
  g_autofree gchar* executable_path = g_file_read_link("/proc/self/exe", nullptr);
  if (executable_path == nullptr) {
    return;
  }

  g_autofree gchar* executable_dir = g_path_get_dirname(executable_path);
  g_autofree gchar* icon_path =
      g_build_filename(executable_dir, "data", "nauterm.png", nullptr);
  if (!g_file_test(icon_path, G_FILE_TEST_IS_REGULAR)) {
    return;
  }

  g_autoptr(GError) error = nullptr;
  gtk_window_set_icon_from_file(window, icon_path, &error);
}

// Implements GApplication::activate.
static void my_application_activate(GApplication* application) {
  MyApplication* self = MY_APPLICATION(application);
  GtkWindow* window =
      GTK_WINDOW(gtk_application_window_new(GTK_APPLICATION(application)));

  gtk_window_set_title(window, "Nauterm");
  set_nauterm_window_icon(window);

  gboolean use_header_bar = TRUE;
#ifdef GDK_WINDOWING_X11
  GdkScreen* screen = gtk_window_get_screen(window);
  if (GDK_IS_X11_SCREEN(screen)) {
    const gchar* wm_name = gdk_x11_screen_get_window_manager_name(screen);
    if (g_strcmp0(wm_name, "GNOME Shell") != 0) {
      use_header_bar = FALSE;
    }
  }
#endif
  if (use_header_bar) {
    GtkHeaderBar* header_bar = GTK_HEADER_BAR(gtk_header_bar_new());
    gtk_widget_show(GTK_WIDGET(header_bar));
    gtk_header_bar_set_title(header_bar, "Nauterm");
    gtk_header_bar_set_show_close_button(header_bar, TRUE);
    gtk_window_set_titlebar(window, GTK_WIDGET(header_bar));
  }

  gtk_window_set_default_size(window, 1280, 720);

  g_autoptr(FlDartProject) project = fl_dart_project_new();
  fl_dart_project_set_dart_entrypoint_arguments(
      project, self->dart_entrypoint_arguments);

  FlView* view = fl_view_new(project);
  GdkRGBA background_color;
  // Background defaults to black, override it here if necessary, e.g. #00000000
  // for transparent.
  gdk_rgba_parse(&background_color, "#000000");
  fl_view_set_background_color(view, &background_color);
  gtk_widget_show(GTK_WIDGET(view));
  gtk_container_add(GTK_CONTAINER(window), GTK_WIDGET(view));

  // Show the window when Flutter renders.
  // Requires the view to be realized so we can start rendering.
  g_signal_connect_swapped(view, "first-frame", G_CALLBACK(first_frame_cb),
                           self);
  gtk_widget_realize(GTK_WIDGET(view));

  fl_register_plugins(FL_PLUGIN_REGISTRY(view));
  setup_file_drop_channel(self, view);
  setup_system_fonts_channel(self, view);
  setup_window_channel(self, view);

  gtk_widget_grab_focus(GTK_WIDGET(view));
}

// Implements GApplication::local_command_line.
static gboolean my_application_local_command_line(GApplication* application,
                                                  gchar*** arguments,
                                                  int* exit_status) {
  MyApplication* self = MY_APPLICATION(application);
  // Strip out the first argument as it is the binary name.
  self->dart_entrypoint_arguments = g_strdupv(*arguments + 1);

  g_autoptr(GError) error = nullptr;
  if (!g_application_register(application, nullptr, &error)) {
    g_warning("Failed to register: %s", error->message);
    *exit_status = 1;
    return TRUE;
  }

  g_application_activate(application);
  *exit_status = 0;

  return TRUE;
}

// Implements GApplication::startup.
static void my_application_startup(GApplication* application) {
  // MyApplication* self = MY_APPLICATION(object);

  // Perform any actions required at application startup.

  G_APPLICATION_CLASS(my_application_parent_class)->startup(application);
}

// Implements GApplication::shutdown.
static void my_application_shutdown(GApplication* application) {
  // MyApplication* self = MY_APPLICATION(object);

  // Perform any actions required at application shutdown.

  G_APPLICATION_CLASS(my_application_parent_class)->shutdown(application);
}

// Implements GObject::dispose.
static void my_application_dispose(GObject* object) {
  MyApplication* self = MY_APPLICATION(object);
  g_clear_object(&self->file_drop_channel);
  g_clear_object(&self->system_fonts_channel);
  g_clear_object(&self->window_channel);
  g_clear_pointer(&self->dart_entrypoint_arguments, g_strfreev);
  G_OBJECT_CLASS(my_application_parent_class)->dispose(object);
}

static void my_application_class_init(MyApplicationClass* klass) {
  G_APPLICATION_CLASS(klass)->activate = my_application_activate;
  G_APPLICATION_CLASS(klass)->local_command_line =
      my_application_local_command_line;
  G_APPLICATION_CLASS(klass)->startup = my_application_startup;
  G_APPLICATION_CLASS(klass)->shutdown = my_application_shutdown;
  G_OBJECT_CLASS(klass)->dispose = my_application_dispose;
}

static void my_application_init(MyApplication* self) {}

MyApplication* my_application_new() {
  // Set the program name to the application ID, which helps various systems
  // like GTK and desktop environments map this running application to its
  // corresponding .desktop file. This ensures better integration by allowing
  // the application to be recognized beyond its binary name.
  g_set_prgname(APPLICATION_ID);

  return MY_APPLICATION(g_object_new(my_application_get_type(),
                                     "application-id", APPLICATION_ID, "flags",
                                     G_APPLICATION_NON_UNIQUE, nullptr));
}
