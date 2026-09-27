import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import Quickshell.Io
import Quickshell.Hyprland
import qs.Commons
import qs.Ui

BarWidget {
  id: root
  moduleName: "local.dofus-workspaces"

  property string statePath: ""
  property string classesPath: ""

  Process {
    id: runtimeDirProbe
    command: ["sh", "-c", "printf %s \"${XDG_RUNTIME_DIR:-/tmp}\""]
    stdout: SplitParser {
      onRead: function(line) {
        var dir = String(line || "").trim()
        if (dir === "") dir = "/tmp"
        root.statePath = dir + "/dofustabs/state.json"
      }
    }
  }

  Process {
    id: dataDirProbe
    command: ["sh", "-c", "printf %s \"${XDG_DATA_HOME:-$HOME/.local/share}\""]
    stdout: SplitParser {
      onRead: function(line) {
        var dir = String(line || "").trim()
        root.classesPath = dir + "/dofustabs/classes.json"
      }
    }
  }

  Component.onCompleted: {
    runtimeDirProbe.running = true
    dataDirProbe.running = true
  }

  property var clients: []
  property var classes: [] // [{name, icon}], empty when unavailable

  readonly property color barForeground: root.bar ? root.bar.barForeground : Color.foreground
  readonly property color focusColor: Qt.colorEqual(Color.accent, barForeground) ? Color.urgent : Color.accent

  readonly property int iconSize: 18

  // The one shared picker's target: the client entry (from `clients`) it is
  // anchored to, or null when closed. Kept as a single property (rather
  // than per-cell state) so right-clicking a different icon simply re-points
  // the same popup instead of opening a second one.
  property var pickerTarget: null
  property Item pickerAnchor: null

  // "" (closed), "accounts" (choose which running client to edit) or
  // "classes" (the class list for `pickerTarget`). Right-clicking an icon
  // jumps straight to "classes" and closes on choice; the left-click button
  // opens "accounts" and returns to it after a choice so the user can keep
  // going through the other running clients.
  property string pickerStep: ""
  property bool pickerFromAccounts: false
  // "Saved" confirmation shown under the name field after a successful
  // save; cleared as soon as the field is edited again.
  property bool pickerNameSaved: false
  // "Close all accounts and launcher" needs a second click within this
  // window to actually run; cleared whenever the picker leaves the
  // "accounts" step or closes.
  property bool quitAllConfirming: false

  readonly property bool needsClassAssignment: {
    for (var i = 0; i < root.clients.length; i++) {
      var c = root.clients[i]
      if (!c.class || c.class === "" || root.isFallbackName(c)) return true
    }
    return false
  }

  function clientLabel(entry) {
    if (!entry) return ""
    return entry.name && entry.name !== "" ? entry.name : ("Account " + entry.account)
  }

  // True when `entry.name` is only the CLI's generated placeholder
  // ("Account <id>", see crates/cli/src/main.rs) rather than a name the
  // user actually set.
  function isFallbackName(entry) {
    if (!entry || !entry.name || entry.name === "") return true
    return entry.name === ("Account " + entry.account)
  }

  function tooltipFor(entry) {
    var hasName = !root.isFallbackName(entry)
    var hasClass = entry.class && entry.class !== ""
    var label = hasName ? entry.name : ("Account " + entry.account)
    if (hasClass) return label + " (" + entry.class + ")"
    return label + " - right click to choose the class"
  }

  function focusWorkspace(id) {
    if (!root.bar) return
    root.bar.run("hyprctl dispatch " + Util.shellQuote("hl.dsp.focus({ workspace = \"" + id + "\" })"))
  }

  // Finds the current `clients` entry for a stale `pickerTarget` snapshot
  // (matched by account, since a client's slot/workspace can change under
  // reconciliation while the picker is open), so the popup always reflects
  // live data and closes itself once the account is gone.
  function liveClientFor(entry) {
    if (!entry) return null
    for (var i = 0; i < root.clients.length; i++) {
      if (root.clients[i].account === entry.account) return root.clients[i]
    }
    return null
  }

  function openPicker(anchorItem, entry) {
    root.pickerAnchor = anchorItem
    root.pickerTarget = entry
    root.pickerStep = "classes"
    root.pickerFromAccounts = false
  }

  // Left-click entry point on the assign-class button: opens the
  // "choose account" step anchored to the button itself.
  function openAccountsPicker(anchorItem) {
    root.pickerAnchor = anchorItem
    root.pickerTarget = null
    root.pickerStep = "accounts"
    root.pickerFromAccounts = false
  }

  // Clicking a row in the "accounts" step: switch to the class list for
  // that client, remembering to return to "accounts" afterwards.
  function selectAccountForClass(entry) {
    root.pickerTarget = entry
    root.pickerStep = "classes"
    root.pickerFromAccounts = true
    root.quitAllConfirming = false
    quitAllTimer.stop()
  }

  function closePicker() {
    root.pickerTarget = null
    root.pickerAnchor = null
    root.pickerStep = ""
    root.pickerFromAccounts = false
    root.quitAllConfirming = false
    quitAllTimer.stop()
  }

  // First click arms the confirmation (auto-reverts after 4s); a second
  // click while armed runs the quit command and closes the popup.
  function onQuitAllClicked() {
    if (root.quitAllConfirming) {
      quitAllTimer.stop()
      root.quitAllConfirming = false
      if (root.bar) root.bar.run("~/.local/bin/dofustabs quit")
      root.closePicker()
    } else {
      root.quitAllConfirming = true
      quitAllTimer.restart()
    }
  }

  Timer {
    id: quitAllTimer
    interval: 4000
    onTriggered: root.quitAllConfirming = false
  }

  // PopupCard dismisses itself (outside click) through `owner.close()`.
  // Without this it would assign `open = false` directly, breaking the
  // `open: root.pickerStep !== ""` binding so the popup could never reopen.
  function close() {
    root.closePicker()
  }

  // Entering the "classes" step (either path) prefills the name field from
  // live data and moves focus to it when we got here through the cog's
  // "accounts" list; a direct right click on an icon leaves focus on the
  // class list instead (so right click + click class stays two actions).
  onPickerStepChanged: {
    if (root.pickerStep === "classes") {
      root.resetNameField()
      Qt.callLater(function() {
        if (root.pickerFromAccounts) nameField.forceActiveFocus()
        else pickerColumn.forceActiveFocus()
      })
    }
  }

  function resetNameField() {
    var live = root.liveClientFor(root.pickerTarget)
    nameField.text = (live && !root.isFallbackName(live)) ? live.name : ""
    root.pickerNameSaved = false
  }

  // Runs the CLI to label the account's active character (or the account
  // itself, when it has no resolved character yet) with a typed name, then
  // shows the "Saved" confirmation. Unlike applyClassChoice, this never
  // closes the popup or leaves the step.
  function saveNameChoice() {
    var live = root.liveClientFor(root.pickerTarget)
    if (!root.bar || !live) return
    var value = nameField.text.trim().slice(0, 40)
    var bin = "~/.local/bin/dofustabs"
    var cmd
    if (live.character && live.character !== "") {
      cmd = bin + " character set --account " + Util.shellQuote(String(live.account))
        + " --character " + Util.shellQuote(String(live.character))
        + (value === "" ? " --clear-name" : (" --name " + Util.shellQuote(value)))
    } else {
      cmd = bin + " accounts set " + Util.shellQuote(String(live.account))
        + (value === "" ? " --clear-name" : (" --name " + Util.shellQuote(value)))
    }
    root.bar.run(cmd)
    nameField.text = value
    root.pickerNameSaved = true
  }

  // Runs the CLI to label the account's active character (or the account
  // itself, when it has no resolved character yet), then closes. The icon
  // updates on its own: the CLI calls `dofustabs.refresh()`, which rewrites
  // state.json, which this widget already watches.
  function applyClassChoice(entry, className) {
    if (!root.bar || !entry) return
    var bin = "~/.local/bin/dofustabs"
    var cmd
    if (entry.character && entry.character !== "") {
      cmd = bin + " character set --account " + Util.shellQuote(String(entry.account))
        + " --character " + Util.shellQuote(String(entry.character))
        + (className === null ? " --clear-class" : (" --class " + Util.shellQuote(className)))
    } else {
      cmd = bin + " accounts set " + Util.shellQuote(String(entry.account))
        + (className === null ? " --clear-class" : (" --class " + Util.shellQuote(className)))
    }
    root.bar.run(cmd)
    if (root.pickerFromAccounts) {
      // Stay open, back at "accounts", so the user can continue with the
      // next client.
      root.pickerTarget = null
      root.pickerStep = "accounts"
      root.pickerFromAccounts = false
    } else {
      root.closePicker()
    }
  }

  function parseState(text) {
    var parsed
    try {
      parsed = JSON.parse(text)
    } catch (e) {
      root.clients = []
      return
    }

    if (!parsed || !Array.isArray(parsed.clients)) {
      root.clients = []
      return
    }

    root.clients = parsed.clients

    // Every client quit while the picker was open (button hides itself too).
    if (root.clients.length === 0 && root.pickerStep !== "") {
      root.closePicker()
      return
    }

    // The client the picker targets may have just closed.
    if (root.pickerStep === "classes" && root.pickerTarget && !root.liveClientFor(root.pickerTarget)) {
      if (root.pickerFromAccounts) {
        root.pickerTarget = null
        root.pickerStep = "accounts"
        root.pickerFromAccounts = false
      } else {
        root.closePicker()
      }
    }
  }

  function parseClasses(text) {
    var parsed
    try {
      parsed = JSON.parse(text)
    } catch (e) {
      root.classes = []
      return
    }
    root.classes = Array.isArray(parsed) ? parsed : []
  }

  FileView {
    id: stateFile
    path: root.statePath
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: root.parseState(text())
  }

  FileView {
    id: classesFile
    path: root.classesPath
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: root.parseClasses(text())
  }

  implicitWidth: row.implicitWidth
  implicitHeight: row.implicitHeight

  RowLayout {
    id: row
    anchors.fill: parent
    spacing: Style.space(1)

    // Visible entry point for the class picker (right-click on an icon does
    // the same thing, but is undiscoverable). Nerd Font cog, same glyph
    // already used by other bar/panel widgets (e.g. akitaonrails.ai-usagebar,
    // io.github.feartheploto.musica) so it's confirmed to render in this font.
    WidgetButton {
      id: assignButton
      bar: root.bar
      visible: root.clients.length > 0
      Layout.preferredHeight: root.barSize
      text: "󰒓"
      tooltipText: "Assign names and class icons"
      active: root.needsClassAssignment
      activeColor: root.focusColor
      dimmed: !root.needsClassAssignment
      onPressed: function(button) {
        if (button === Qt.LeftButton) root.openAccountsPicker(assignButton)
      }
    }

    Repeater {
      model: root.clients

      Item {
        id: cell
        required property var modelData

        readonly property bool focused: Hyprland.focusedWorkspace !== null
          && Hyprland.focusedWorkspace.id === modelData.workspace

        Layout.preferredWidth: root.barSize
        Layout.preferredHeight: root.barSize

        Image {
          id: icon
          anchors.centerIn: parent
          width: root.iconSize
          height: root.iconSize
          source: modelData.icon ? ("file://" + modelData.icon) : ""
          fillMode: Image.PreserveAspectFit
          asynchronous: true
          sourceSize.width: width * Screen.devicePixelRatio
          sourceSize.height: height * Screen.devicePixelRatio
          opacity: cell.focused ? 1 : 0.5
        }

        Rectangle {
          visible: cell.focused
          anchors.horizontalCenter: parent.horizontalCenter
          anchors.bottom: parent.bottom
          width: root.iconSize
          height: 2
          radius: 1
          color: root.focusColor
        }

        MouseArea {
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          acceptedButtons: Qt.LeftButton | Qt.RightButton
          onEntered: {
            if (!root.bar) return
            root.bar.showTooltip(cell, root.tooltipFor(modelData))
          }
          onExited: {
            if (root.bar) root.bar.hideTooltip(cell)
          }
          onClicked: function(mouse) {
            if (mouse.button === Qt.RightButton) {
              if (root.bar) root.bar.hideTooltip(cell)
              root.openPicker(cell, modelData)
            } else {
              root.focusWorkspace(modelData.workspace)
            }
          }
        }
      }
    }
  }

  // One shared picker, anchored to whichever icon was right-clicked last
  // (`root.pickerAnchor`); right-clicking a different icon re-anchors it
  // instead of opening a second popup.
  PopupCard {
    id: picker
    anchorItem: root.pickerAnchor || root
    owner: root
    bar: root.bar
    open: root.pickerStep !== ""
    triggerMode: "click"
    contentWidth: picker.fittedContentWidth(Style.space(240))
    // Follows whichever step is currently visible, not a fixed split.
    contentHeight: picker.fittedContentHeight(
      pickerHeader.implicitHeight + Style.space(4)
        + (root.pickerStep === "accounts"
            ? accountsColumn.implicitHeight
            : (nameColumn.implicitHeight + Style.space(4) + rowsColumn.implicitHeight)),
      Style.space(400))

    // HyprlandFocusGrab (inside PopupCard) only ever grabs input while
    // `open` is true; Hyprland's own keyboard focus (`activewindow`) is
    // never changed by opening the popup, so clearing the grab (outside
    // click, Escape, or a selection) hands keyboard input straight back to
    // whichever window Hyprland already considered focused underneath -
    // the game window, if that's what it was. There is no separate
    // "restore focus" step needed here.
    onOpenChanged: {
      if (open) Qt.callLater(function() { pickerColumn.forceActiveFocus() })
    }

    Column {
      id: pickerColumn
      anchors.fill: parent
      spacing: Style.space(4)
      focus: true

      Keys.onEscapePressed: root.closePicker()

      Text {
        id: pickerHeader
        textFormat: Text.PlainText
        width: parent.width
        elide: Text.ElideRight
        text: root.pickerStep === "accounts"
          ? "Assign class icons"
          : root.clientLabel(root.liveClientFor(root.pickerTarget) || root.pickerTarget || {})
        color: root.barForeground
        font.family: (root.bar && root.bar.fontFamily) ? root.bar.fontFamily : Style.font.family
        font.pixelSize: Style.font.body
        font.bold: true
      }

      // "Choose account" step: one row per running client, showing its
      // current icon/label/class, plus a trailing "Done" row.
      Column {
        id: accountsColumn
        visible: root.pickerStep === "accounts"
        width: parent.width
        spacing: Style.space(2)

        Repeater {
          model: root.clients

          Rectangle {
            id: accountRow
            required property var modelData
            readonly property bool hasClass: accountRow.modelData.class && accountRow.modelData.class !== ""
            readonly property bool hasName: !root.isFallbackName(accountRow.modelData)

            width: accountsColumn.width
            height: accountRow.hasName ? Style.space(34) : Style.space(24)
            color: accountHover.hovered
              ? Style.hoverFillFor(root.barForeground, root.focusColor)
              : "transparent"
            radius: Style.cornerRadius

            Image {
              id: accountIcon
              anchors.verticalCenter: parent.verticalCenter
              anchors.left: parent.left
              anchors.leftMargin: Style.space(6)
              width: root.iconSize
              height: root.iconSize
              source: accountRow.modelData.icon ? ("file://" + accountRow.modelData.icon) : ""
              fillMode: Image.PreserveAspectFit
              asynchronous: true
            }

            // Name prominent; "Account <id>" underneath in the smaller font
            // once a real name is set, so accounts stay distinguishable.
            Column {
              anchors.verticalCenter: parent.verticalCenter
              anchors.left: accountIcon.right
              anchors.leftMargin: Style.space(8)
              anchors.right: accountClassLabel.left
              anchors.rightMargin: Style.space(6)
              spacing: 0

              Text {
                textFormat: Text.PlainText
                width: parent.width
                elide: Text.ElideRight
                text: accountRow.hasName ? accountRow.modelData.name : ("Account " + accountRow.modelData.account)
                color: root.barForeground
                font.family: (root.bar && root.bar.fontFamily) ? root.bar.fontFamily : Style.font.family
                font.pixelSize: Style.font.body
              }

              Text {
                visible: accountRow.hasName
                textFormat: Text.PlainText
                width: parent.width
                elide: Text.ElideRight
                text: "Account " + accountRow.modelData.account
                color: Qt.darker(root.barForeground, 1.4)
                font.family: (root.bar && root.bar.fontFamily) ? root.bar.fontFamily : Style.font.family
                font.pixelSize: Style.font.bodySmall
              }
            }

            Text {
              id: accountClassLabel
              textFormat: Text.PlainText
              anchors.verticalCenter: parent.verticalCenter
              anchors.right: parent.right
              anchors.rightMargin: Style.space(6)
              text: accountRow.hasClass ? accountRow.modelData.class : "not set"
              color: root.focusColor
              font.family: (root.bar && root.bar.fontFamily) ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.bodySmall
              font.italic: !accountRow.hasClass
            }

            HoverHandler {
              id: accountHover
            }

            MouseArea {
              anchors.fill: parent
              cursorShape: Qt.PointingHandCursor
              onClicked: root.selectAccountForClass(accountRow.modelData)
            }
          }
        }

        Rectangle {
          id: doneRow
          width: accountsColumn.width
          height: Style.space(24)
          color: doneHover.hovered
            ? Style.hoverFillFor(root.barForeground, root.focusColor)
            : "transparent"
          radius: Style.cornerRadius

          Text {
            textFormat: Text.PlainText
            anchors.verticalCenter: parent.verticalCenter
            anchors.left: parent.left
            anchors.leftMargin: Style.space(6)
            text: "Done"
            color: Qt.darker(root.barForeground, 1.2)
            font.family: (root.bar && root.bar.fontFamily) ? root.bar.fontFamily : Style.font.family
            font.pixelSize: Style.font.body
            font.italic: true
          }

          HoverHandler {
            id: doneHover
          }

          MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: root.closePicker()
          }
        }

        Rectangle {
          id: quitAllRow
          width: accountsColumn.width
          height: Style.space(24)
          color: quitAllHover.hovered
            ? Style.hoverFillFor(root.barForeground, Color.urgent)
            : "transparent"
          radius: Style.cornerRadius

          Text {
            textFormat: Text.PlainText
            anchors.verticalCenter: parent.verticalCenter
            anchors.left: parent.left
            anchors.leftMargin: Style.space(6)
            text: root.quitAllConfirming
              ? "Click again to confirm"
              : "Close all accounts and launcher"
            color: Color.urgent
            font.family: (root.bar && root.bar.fontFamily) ? root.bar.fontFamily : Style.font.family
            font.pixelSize: Style.font.body
          }

          HoverHandler {
            id: quitAllHover
          }

          MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: root.onQuitAllClicked()
          }
        }
      }

      // Name editor for the "classes" step's target client: prefilled from
      // its current name (left empty when that's only the CLI's "Account
      // <id>" placeholder), saved independently of the class choice below
      // so both can be set in one visit.
      Column {
        id: nameColumn
        visible: root.pickerStep === "classes"
        width: parent.width
        spacing: Style.space(2)

        Text {
          textFormat: Text.PlainText
          text: "Name"
          color: Qt.darker(root.barForeground, 1.2)
          font.family: (root.bar && root.bar.fontFamily) ? root.bar.fontFamily : Style.font.family
          font.pixelSize: Style.font.bodySmall
        }

        Row {
          id: nameRow
          width: nameColumn.width
          spacing: Style.space(6)

          TextField {
            id: nameField
            width: nameColumn.width - saveNameButton.width - nameRow.spacing
            placeholderText: "Character name"
            foreground: root.barForeground
            maximumLength: 40
            verticalPadding: Style.space(4)

            onAccepted: root.saveNameChoice()
            onTextEdited: root.pickerNameSaved = false
            Keys.onEscapePressed: root.closePicker()
          }

          Rectangle {
            id: saveNameButton
            width: Math.max(Style.space(40), saveNameLabel.implicitWidth + Style.space(12))
            height: nameField.implicitHeight
            color: saveNameHover.hovered
              ? Style.hoverFillFor(root.barForeground, root.focusColor)
              : "transparent"
            radius: Style.cornerRadius

            Text {
              id: saveNameLabel
              anchors.centerIn: parent
              textFormat: Text.PlainText
              text: "Save"
              color: root.barForeground
              font.family: (root.bar && root.bar.fontFamily) ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.body
            }

            HoverHandler {
              id: saveNameHover
            }

            MouseArea {
              anchors.fill: parent
              cursorShape: Qt.PointingHandCursor
              onClicked: root.saveNameChoice()
            }
          }
        }

        Text {
          visible: root.pickerNameSaved
          textFormat: Text.PlainText
          text: "Saved"
          color: root.focusColor
          font.family: (root.bar && root.bar.fontFamily) ? root.bar.fontFamily : Style.font.family
          font.pixelSize: Style.font.bodySmall
        }
      }

      // "Classes" step: the existing class-list content, shared by the
      // right-click-on-icon path and the "accounts" step's row click.
      Column {
        id: rowsColumn
        visible: root.pickerStep === "classes"
        width: parent.width
        spacing: Style.space(2)

        // Unavailable/malformed classes.json: one disabled informational
        // row, no picking, no QML errors.
        Rectangle {
          visible: root.classes.length === 0
          width: rowsColumn.width
          height: Style.space(24)
          color: "transparent"
          Text {
            anchors.verticalCenter: parent.verticalCenter
            anchors.left: parent.left
            text: "Class list unavailable, run dofustabs install"
            color: Qt.darker(root.barForeground, 1.4)
            font.family: (root.bar && root.bar.fontFamily) ? root.bar.fontFamily : Style.font.family
            font.pixelSize: Style.font.bodySmall
            font.italic: true
          }
        }

        Repeater {
          model: root.classes

          Rectangle {
            id: classRow
            required property var modelData
            readonly property var liveEntry: root.liveClientFor(root.pickerTarget)
            readonly property bool isCurrent: liveEntry && liveEntry.class === modelData.name

            width: rowsColumn.width
            height: Style.space(24)
            color: rowHover.hovered
              ? Style.hoverFillFor(root.barForeground, root.focusColor)
              : (isCurrent ? Qt.darker(root.focusColor, 2.2) : "transparent")
            radius: Style.cornerRadius

            Image {
              id: classIcon
              anchors.verticalCenter: parent.verticalCenter
              anchors.left: parent.left
              anchors.leftMargin: Style.space(6)
              width: root.iconSize
              height: root.iconSize
              source: classRow.modelData.icon ? ("file://" + classRow.modelData.icon) : ""
              fillMode: Image.PreserveAspectFit
              asynchronous: true
            }

            Text {
              textFormat: Text.PlainText
              anchors.verticalCenter: parent.verticalCenter
              anchors.left: classIcon.right
              anchors.leftMargin: Style.space(8)
              anchors.right: parent.right
              anchors.rightMargin: Style.space(6)
              elide: Text.ElideRight
              text: classRow.modelData.name
              color: classRow.isCurrent
                ? Style.hoverStateColor(root.barForeground, root.focusColor)
                : root.barForeground
              font.family: (root.bar && root.bar.fontFamily) ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.body
            }

            HoverHandler {
              id: rowHover
            }

            MouseArea {
              anchors.fill: parent
              cursorShape: Qt.PointingHandCursor
              onClicked: {
                var live = root.liveClientFor(root.pickerTarget)
                if (live) root.applyClassChoice(live, classRow.modelData.name)
              }
            }
          }
        }

        Rectangle {
          id: clearRow
          visible: root.classes.length > 0
          width: rowsColumn.width
          height: Style.space(24)
          color: clearHover.hovered
            ? Style.hoverFillFor(root.barForeground, root.focusColor)
            : "transparent"
          radius: Style.cornerRadius

          Text {
            textFormat: Text.PlainText
            anchors.verticalCenter: parent.verticalCenter
            anchors.left: parent.left
            anchors.leftMargin: Style.space(6)
            text: "Clear class"
            color: Qt.darker(root.barForeground, 1.2)
            font.family: (root.bar && root.bar.fontFamily) ? root.bar.fontFamily : Style.font.family
            font.pixelSize: Style.font.body
            font.italic: true
          }

          HoverHandler {
            id: clearHover
          }

          MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: {
              var live = root.liveClientFor(root.pickerTarget)
              if (live) root.applyClassChoice(live, null)
            }
          }
        }
      }
    }
  }
}
