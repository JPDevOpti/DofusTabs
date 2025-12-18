# DofusTabs

Aplicación de escritorio para Windows diseñada específicamente para facilitar la gestión de múltiples cuentas de Dofus simultáneamente. Permite cambiar rápidamente entre personajes usando atajos de teclado globales y un overlay visual flotante.

## Objetivo

DofusTabs está orientado al uso cómodo de multicuenta en Dofus, eliminando la necesidad de hacer clic en la barra de tareas o usar Alt+Tab repetidamente. La aplicación detecta automáticamente todas las ventanas de Dofus abiertas y proporciona controles rápidos para navegar entre ellas de forma eficiente.

## Requisitos del Sistema

- Windows 10/11
- .NET 8.0 Runtime
- Dofus instalado y en ejecución

## Instalación

1. Descarga la última versión desde las releases
2. Extrae el archivo ZIP en cualquier ubicación
3. Ejecuta `DofusTabs.exe`

No requiere instalación adicional. La aplicación es portátil y puede ejecutarse desde cualquier carpeta.

## Características Principales

### Detección Automática de Ventanas
- Detecta automáticamente todas las ventanas de Dofus abiertas en el sistema
- Identifica el nombre del personaje y la clase desde el título de la ventana
- Muestra iconos de clase para cada personaje
- Actualización manual mediante botón de refresco

### Atajos de Teclado Globales
- **Siguiente ventana**: Cambia al siguiente personaje habilitado (por defecto: Alt + Tab)
- **Ventana anterior**: Cambia al personaje anterior habilitado (por defecto: Alt + Shift + Tab)
- **Atajos individuales**: Asigna una combinación de teclas única para acceder directamente a cada personaje
- Los atajos funcionan globalmente, incluso cuando la aplicación está minimizada
- Configuración personalizable de todas las combinaciones de teclas

### Overlay Flotante
- Ventana flotante que permanece siempre visible sobre otras aplicaciones
- Lista visual de todos los personajes con sus iconos de clase
- Clic directo en cualquier personaje para cambiar a su ventana
- Dos modos de visualización:
  - **Modo completo**: Muestra iconos y nombres de personajes
  - **Modo compacto**: Solo iconos para ocupar menos espacio en pantalla
- Posición personalizable y se guarda automáticamente
- Puede ocultarse/mostrarse cuando sea necesario

### Gestión de Ventanas
- **Habilitar/Deshabilitar**: Marca qué ventanas participan en la rotación de atajos
- **Reordenamiento**: Arrastra y suelta personajes para cambiar el orden de navegación
- El orden se sincroniza automáticamente entre la lista principal y el overlay
- Contador de ventanas detectadas y habilitadas

### Sistema de Bandeja
- Se minimiza a la bandeja del sistema sin ocupar espacio en la barra de tareas
- Acceso rápido mediante menú contextual:
  - Mostrar ventana principal
  - Salir de la aplicación
- Icono siempre visible para acceso rápido

### Persistencia de Configuración
- Guarda automáticamente todos los ajustes:
  - Atajos de teclado globales e individuales
  - Estado de ventanas (habilitadas/deshabilitadas)
  - Orden personalizado de los personajes
  - Posición y modo del overlay
  - Visibilidad del overlay
- La configuración se carga automáticamente al iniciar

## Uso

### Inicio Rápido

1. Abre las ventanas de Dofus que desees gestionar
2. Ejecuta DofusTabs
3. La aplicación detectará automáticamente todas las ventanas abiertas
4. Usa los atajos de teclado predeterminados para navegar entre personajes

### Configuración de Atajos Globales

1. Haz clic en el campo de texto "Siguiente ventana" o "Ventana anterior"
2. Presiona la combinación de teclas deseada (ej: Ctrl + Alt + D)
3. El atajo se registra automáticamente y funciona de forma global
4. Para eliminar un atajo, usa el botón "X" junto al campo

### Configuración de Atajos Individuales

1. Haz clic en el campo "Atajo" de cualquier personaje en la lista
2. Presiona la combinación de teclas que desees asignar
3. Si la combinación ya está en uso, se reasignará al nuevo personaje
4. Cada personaje puede tener su propio atajo directo

### Uso del Overlay

1. Haz clic en "Mostrar Overlay" para activar la ventana flotante
2. Arrastra el overlay a la posición deseada en tu pantalla
3. Haz clic en cualquier personaje del overlay para cambiar a su ventana
4. Usa el botón "←/→" para alternar entre modo completo y compacto
5. El overlay permanece visible incluso mientras juegas

### Reordenar Personajes

1. En la lista principal, arrastra cualquier personaje
2. Suéltalo en la posición deseada
3. El nuevo orden se aplica tanto en la lista como en el overlay
4. Este orden determina la secuencia de navegación con atajos

### Habilitar/Deshabilitar Ventanas

1. Usa los checkboxes en la columna "Habilitada"
2. Solo las ventanas habilitadas participan en la rotación de atajos
3. Las ventanas deshabilitadas siguen visibles en la lista pero se saltan
4. Útil para excluir temporalmente personajes sin cerrar sus ventanas

## Consejos de Uso

- **Minimiza la aplicación**: Funciona desde la bandeja del sistema sin ocupar espacio
- **Actualiza la lista**: Si abres nuevas ventanas de Dofus, usa el botón "Refrescar"
- **Modo compacto**: Ideal para pantallas pequeñas o durante combate
- **Atajos consistentes**: Usa combinaciones coherentes (ej: Ctrl + 1, Ctrl + 2, etc.)
- **Orden personalizado**: Ordena los personajes según tu estrategia de juego

## Estructura del Proyecto

```
DofusTabs/
├── Core/
│   ├── HotkeyManager.cs      # Gestión de atajos de teclado globales
│   └── WindowManager.cs      # Detección y control de ventanas de Dofus
├── UI/
│   ├── MainWindow.xaml       # Ventana principal de la aplicación
│   ├── MainWindow.xaml.cs
│   ├── OverlayWindow.xaml    # Ventana flotante del overlay
│   └── OverlayWindow.xaml.cs
├── Utils/
│   └── SettingsManager.cs    # Persistencia de configuración
├── Resources/
│   ├── Icons/                # Iconos de clases de Dofus
│   ├── Styles.xaml          # Estilos visuales
│   ├── Icon.ico
│   └── Icon.png
└── DofusTabs.csproj
```

## Tecnologías

- **.NET 8.0**: Framework principal
- **WPF (Windows Presentation Foundation)**: Interfaz de usuario
- **Windows API**: Detección y control de ventanas
- **System.Management**: Gestión de procesos

## Limitaciones Conocidas

- Solo funciona en Windows
- Requiere que Dofus esté ejecutándose para detectar ventanas
- Los nombres de personajes y clases deben estar en el formato estándar del título de ventana de Dofus
- Los atajos globales pueden entrar en conflicto con otros programas

## Soporte de Clases

Iconos disponibles para todas las clases de Dofus:
Aniripsa, Anutrof, Feca, Forjalanza, Hipermago, Ocra, Osamodas, Pandawa, Sacrógrito, Sadida, Selotrop, Sram, Steamer, Tymador, Uginak, Xelor, Yopuka, Zobal, Zurcar

## Licencia

Este proyecto es software libre para uso personal.
