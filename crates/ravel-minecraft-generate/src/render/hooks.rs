//! Renders Hook.java, including library loading and bootstrap-owned native declarations.

use std::fmt::Write as _;

use crate::inputs::Adapter;
use crate::{HOST_LIBRARY, LIBRARY_PROPERTY};

use super::shape::Shape;

/// The library load the generated `Hook` performs, and why it names a file.
///
/// Uses an absolute path rather than launcher-managed library search paths.
fn library_load_java() -> String {
    format!(
        "\
        \x20   /** Native library carrying the generated exports. */\n\
         \x20   public static final String LIBRARY = \"{HOST_LIBRARY}\";\n\n\
         \x20   /** System property naming the file to load the library from. */\n\
         \x20   public static final String LIBRARY_PROPERTY = \"{LIBRARY_PROPERTY}\";\n\n\
         \x20   static {{\n\
         \x20       loadLibrary();\n\
         \x20   }}\n\n\
         \x20   // JNI binds a loaded library to the class loader that loaded the class\n\
         \x20   // whose code called System.load, and resolves native methods only against\n\
         \x20   // that loader's copy of the declaring class. Loading it here, from the\n\
         \x20   // declaring class's own initializer, makes those two loaders the same one\n\
         \x20   // by construction rather than by convention, so a second copy of this class\n\
         \x20   // under another loader cannot bind to a library this one loaded. What binds\n\
         \x20   // the library is which class's code called, not which overload it called.\n\
         \x20   //\n\
         \x20   // The file is named rather than searched because the launcher owns\n\
         \x20   // java.library.path: it appends its own after the instance's and the last\n\
         \x20   // one wins, and it rebuilds the directory that path names on every launch.\n\
         \x20   // A library reached by that search is one another program decides the\n\
         \x20   // existence of.\n\
         \x20   //\n\
         \x20   // The load is a restricted method under JEP 472. It is exactly the\n\
         \x20   // operation this class exists to perform, and the launch enables native\n\
         \x20   // access for it, so the warning is acknowledged here rather than silenced\n\
         \x20   // across the whole class.\n\
         \x20   @SuppressWarnings(\"restricted\")\n\
         \x20   private static void loadLibrary() {{\n\
         \x20       String named = System.getProperty(LIBRARY_PROPERTY);\n\
         \x20       if (named == null || named.isEmpty()) {{\n\
         \x20           throw new IllegalStateException(\n\
         \x20                   \"-D\"\n\
         \x20                           + LIBRARY_PROPERTY\n\
         \x20                           + \" is unset, so nothing names the \"\n\
         \x20                           + LIBRARY\n\
         \x20                           + \" library this class loads\");\n\
         \x20       }}\n\
         \x20       java.io.File file = new java.io.File(named);\n\
         \x20       if (!file.isFile()) {{\n\
         \x20           throw new IllegalStateException(\n\
         \x20                   \"-D\"\n\
         \x20                           + LIBRARY_PROPERTY\n\
         \x20                           + \" names \"\n\
         \x20                           + named\n\
         \x20                           + \", which is not a file, so the \"\n\
         \x20                           + LIBRARY\n\
         \x20                           + \" library cannot be loaded\");\n\
         \x20       }}\n\
         \x20       System.load(file.getAbsolutePath());\n\
         \x20   }}\n\n"
    )
}

/// Renders the generated Java hook class.
pub(crate) fn hook_java(adapter: &Adapter, shapes: &[Shape]) -> String {
    let mut text = String::with_capacity(shapes.len() * 384 + 2048);
    let load = library_load_java();
    writeln!(
        text,
        "// Generated from the pinned Minecraft {} inputs. Do not edit.\n\
     package dev.ravel.jvm;\n\n\
     /**\n\
     \x20* Hook surface the agent injects calls to.\n\
     \x20*\n\
     \x20* <p>Each reviewed native observation receives the target's arguments.\n\
     \x20* Observation returns no verdict and does not decide whether the target runs.\n\
     \x20* Legacy semantic callbacks are separate: command dispatch may return an\n\
     \x20* integer result, and the reviewed item-fire seam may return false early.\n\
     \x20* Completion observers run only before normal returns, preserving the\n\
     \x20* target's original value. Observer failures propagate to the caller.\n\
     \x20*/\n\
     public final class Hook {{\n\n\
    {load}\
     \x20   /** Targets the agent rewrites, in reviewed inventory order. */\n\
     \x20   public static final HookTarget[] TARGETS = {{",
        adapter.minecraft
    )
    .expect("writing to a String cannot fail");

    for shape in shapes {
        writeln!(
            text,
            "        new HookTarget(\n\
         \x20               \"{}\",\n\
         \x20               \"{}\",\n\
         \x20               \"{}\",\n\
         \x20               {},\n\
         \x20               \"{}\",\n\
         \x20               \"{}\"),",
            shape.runtime_class,
            shape.runtime_member,
            shape.runtime_descriptor,
            shape.is_static,
            shape.java_method,
            shape.hook_descriptor()
        )
        .expect("writing to a String cannot fail");
    }

    writeln!(
        text,
        "    }};\n\n\
     \x20   private Hook() {{\n\
     \x20   }}\n\n\
     \x20   /** Forces this class to initialize, and with it the library load. */\n\
     \x20   public static void initialize() {{\n\
     \x20   }}\n"
    )
    .expect("writing to a String cannot fail");

    text.push_str(&adapter_callbacks(adapter.is_legacy()));

    for shape in shapes {
        let declared = shape
            .parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| format!("{} argument{index}", parameter.java()))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            text,
            "    /** {} */\n\
         \x20   public static native void {}({declared});\n",
            shape.capability, shape.java_method
        )
        .expect("writing to a String cannot fail");
    }

    // Written rather than generated, like the count below them: these belong to
    // the bootstrap rather than to any version's hook surface. They are declared
    // here because Ravel has one exported symbol surface and it is this class —
    // a screen with natives of its own would be a second one.
    writeln!(text, "{}", written_declarations()).expect("writing to a String cannot fail");
    text
}

/// Keeps adapter policy in Java while preserving the generated native observations.
fn adapter_callbacks(legacy: bool) -> String {
    let enter = if legacy {
        "LegacyRuntime.enter(id, receiver, first, second, number);\n\
         \x20       if (id.equals(\"legacyWorldOpen\") || id.equals(\"legacyWorldOpenIntegrated\")) LegacyPersistence.open(receiver, (String) first);\n\
         \x20       if (id.equals(\"legacyWorldSave\")) LegacyPersistence.save(receiver);"
    } else {
        ""
    };
    let exit = if legacy {
        "if (id.equals(\"legacyScreenOpen\")) LegacyScreens.onOpened(first);\n\
         \x20       if (id.equals(\"legacyTitleInit\")) LegacyScreens.onOpened(receiver);\n\
         \x20       LegacyRuntime.exit(id, receiver, first, second, number, result, resultNumber);\n\
         \x20       if (id.equals(\"legacyWorldOpen\") || id.equals(\"legacyWorldOpenIntegrated\")) LegacyPersistence.loaded(receiver);"
    } else {
        ""
    };
    let command = if legacy {
        "return LegacyCommand.execute(handler, sender, text);"
    } else {
        "return null;"
    };
    let refusal = if legacy {
        "return LegacyRuntime.refuse(id, receiver, source);"
    } else {
        "return false;"
    };
    let natives = if legacy { legacy_declarations() } else { "" };
    let observe = if legacy {
        "LegacyRuntime.observe(id);"
    } else {
        ""
    };
    let palette = palette_callbacks(legacy);
    format!(
        "    /** Whether the reviewed inventory uses the legacy adapter. */\n\
         \x20   public static final boolean LEGACY_ADAPTER = {legacy};\n\n\
         \x20   /** Receiver-aware adapter entry; native observations remain separate. */\n\
         \x20   public static void legacyEnter(String id, Object receiver, Object first, Object second, int number) {{\n\
         \x20       {enter}\n\
         \x20   }}\n\n\
         \x20   /** Successful-return seam with selected original arguments and separate reference/int results. */\n\
         \x20   public static void legacyExit(String id, Object receiver, Object first, Object second, int number, Object result, int resultNumber) {{\n\
         \x20       {exit}\n\
         \x20   }}\n\n\
         \x20   /** Null leaves non-Ravel commands to vanilla. */\n\
         \x20   public static Integer legacyCommand(Object handler, Object sender, String text) {{\n\
         \x20       {command}\n\
         \x20   }}\n\n\
         \x20   /** Refuses only an explicitly supported legacy boolean seam. */\n\
         \x20   public static boolean legacyRefuse(String id, Object receiver, Object source) {{\n\
         \x20       {refusal}\n\
         \x20   }}\n\n\
         \x20   /** Optional proof accounting, separate from native observations and semantics. */\n\
         \x20   public static void legacyObserve(String id) {{\n\
         \x20       {observe}\n\
         \x20   }}\n{palette}{natives}"
    )
}

/// Matches only the reviewed global state registry in the reviewed palette resize body.
fn palette_callbacks(legacy: bool) -> String {
    let size = if legacy {
        "LegacyRuntime.paletteSize(count)"
    } else {
        "count"
    };
    let call = if legacy {
        "owner.equals(LegacyNames.className(\"LEGACY_STATE_IDS\").replace('.', '/'))\n\
         \x20               && (name + \":\" + descriptor).equals(LegacyNames.member(\"LEGACY_STATE_IDS\", \"size\"))"
    } else {
        "false"
    };
    let field = if legacy {
        "owner.equals(LegacyNames.className(\"LEGACY_BLOCK\").replace('.', '/'))\n\
         \x20               && (name + \":\" + descriptor).equals(LegacyNames.member(\"LEGACY_BLOCK\", \"STATE_REGISTRY\"))"
    } else {
        "false"
    };
    format!(
        "    /** Expands only the global palette's addressable state-id range. */\n\
         \x20   public static int legacyPaletteSize(int count) {{ return {size}; }}\n\n\
         \x20   /** Exact reviewed global state registry size invocation. */\n\
         \x20   public static boolean legacyPaletteSizeCall(String owner, String name, String descriptor) {{\n\
         \x20       return {call};\n\
         \x20   }}\n\n\
         \x20   /** Exact reviewed global state registry field read. */\n\
         \x20   public static boolean legacyPaletteField(String owner, String name, String descriptor) {{\n\
         \x20       return {field};\n\
         \x20   }}\n"
    )
}

/// Semantic crossings are separate from the generated target observations.
fn legacy_declarations() -> &'static str {
    "    /** Checked semantic events emitted by the legacy Java adapter. */\n\
     \x20   public static native void ravelLegacyEvent(String kind);\n\
     \x20   public static native void ravelLegacyPacket(String channel, byte[] bytes, boolean toServer);\n\
     \x20   public static native boolean ravelLegacyPacketOwner(String channel);\n\
     \x20   public static native String ravelLegacyCommand(String typed);\n\
     \x20   public static native String ravelLegacyCommands();\n\
     \x20   public static native void ravelInitializeIntegratedServer();\n\
     \x20   public static native String ravelLegacyResourcePaths();\n\
     \x20   public static native byte[] ravelLegacyResource(String path);\n"
}

/// The declarations that belong to the bootstrap rather than to a hook surface.
fn written_declarations() -> &'static str {
    "    /**\n\
         \x20    * Constructs, configures, drives, and installs the composed mod set.\n\
         \x20    *\n\
         \x20    * <p>Called once, from the moment the transformer sees the first\n\
         \x20    * distribution entry point, which is where the running side first\n\
         \x20    * exists as a fact. A premain class-path probe cannot answer it: the\n\
         \x20    * published server jar is a bundler and its real jars are not on any\n\
         \x20    * loader yet.\n\
         \x20    *\n\
         \x20    * @param side the code {@code dev.ravel.jvm.Agent} named the side by\n\
         \x20    * @param profilingRequested whether this launch requested opt-in profiling\n\
         \x20    */\n\
         \x20   public static native void ravelInitialize(int side, boolean profilingRequested);\n\n\
         \x20   /** Scalar renderer lifecycle event; returns the current generation. */\n\
         \x20   public static native long ravelRendererEvent(int event, long generation, long threadId, boolean ownerThread);\n\n\
         \x20   /** Whether completed renderer work still belongs to the active generation. */\n\
         \x20   public static native boolean ravelRendererAccept(long generation);\n\n\
         \x20   /** Bounded batch: [accepted, bytes, mismatches, cells, copyBytes, scratchBoundBytes, admitted, key]. Failure: [-1, code, detailA, detailB, detailC, 0, 0, key]; stale/busy is empty.\n\
         \x20    * Failure codes: 1 generation; 2 lengths(states,light,geometry); 3 state(index,key); 4 geometry(index,flag); 5 inconsistent geometry(stateKey); 6 palette; 7 null arrays(bitmask states=1,light=2,geometry=4). */\n\
         \x20   public static native long[] ravelRendererSnapshot(long generation, long key, int[] states, byte[] light, byte[] geometry);\n\n\
         \x20   /** Mesh work: 0 consumes bounded metadata, 1 current stats, 2 closes/drains, 3 retired stats, 4 peeks retained metadata; these require key=0. 5 acknowledges exact generation/key: [1] released, [0] absent, empty stale/closed/invalid. */\n\
         \x20   public static native long[] ravelRendererWork(int operation, long generation, long key);\n\n\
         \x20   /** Non-consuming Java-owned solid mesh copy; empty for absent/stale/closed/invalid. LE v1: magic 0x354d5652,u32 version,i64 generation/key,u32 status/vertexCount/indexCount/materialCount,u64 visibility (48 bytes); vertices XYZ/face/light (5 bytes), u32 indices, material u32 stateKey/firstIndex/indexCount (12 bytes). Status 0 complete,1 incomplete,2 error; only 0 drawable. At most 1,376,304 bytes. */\n\
         \x20   public static native byte[] ravelRendererMesh(long generation, long key);\n\n\
         \x20   /** Hook invocations the native side has counted. */\n\
         \x20   public static native long nativeInvocations();\n\n\
         \x20   /**\n\
         \x20    * One mod's settings, as the screen draws them.\n\
         \x20    *\n\
         \x20    * <p>There are two batched crossings per settings session and this is\n\
         \x20    * the first: everything a screen needs to build its widgets arrives in\n\
         \x20    * one document, so nothing crosses per widget and nothing per frame.\n\
         \x20    *\n\
         \x20    * <p>Both payloads are length-prefixed fields rather than JSON. A\n\
         \x20    * setting's title, description and value are free-form text, so a\n\
         \x20    * delimited format would need escaping and the screen has no JSON\n\
         \x20    * parser it may borrow; a length says where a field ends whatever is\n\
         \x20    * inside it.\n\
         \x20    *\n\
         \x20    * @param owner the opaque typed route from the mod-list document,\n\
         \x20    *     {@code process:<mod-id>} or {@code integrated-server:<mod-id>};\n\
         \x20    *     a display suffix is never a routing identity\n\
         \x20    * @return the settings document, or {@code null} when this process\n\
         \x20    *     holds no configuration for that owner\n\
         \x20    */\n\
         \x20   public static native String ravelSettingsDocument(String owner);\n\n\
         \x20   /**\n\
         \x20    * Records what the player changed, and answers what was refused.\n\
         \x20    *\n\
         \x20    * <p>The second of the two crossings. A value the owning mod refuses\n\
         \x20    * leaves the previous one in place, and the answer says which keys\n\
         \x20    * those were and why, so the screen can tell the player rather than\n\
         \x20    * letting an edit vanish between one open and the next.\n\
         \x20    *\n\
         \x20    * @param payload the player's values for one owner\n\
         \x20    * @return the refused keys and their reasons, empty when every edit\n\
         \x20    *     landed\n\
         \x20    */\n\
         \x20   public static native String ravelSaveSettings(String payload);\n\n\
         \x20   /** Settings document and save crossings in this process. */\n\
         \x20   public static native String ravelSettingsCrossings();\n\n\
         \x20   /**\n\
         \x20    * The installed mod set, in resolved load order.\n\
         \x20    *\n\
         \x20    * <p>The mod list uses the settings screen's mechanism,\n\
         \x20    * so it is one batched read on the same framing: the whole list and\n\
         \x20    * every failure's attribution arrive together, and nothing crosses\n\
         \x20    * per row or per frame.\n\
         \x20    *\n\
         \x20    * <p>A failure crosses as attribution rather than as a sentence —\n\
         \x20    * kind, surface, dependency chain, the state it failed in — and the\n\
         \x20    * screen composes what it shows from those. The detail text has\n\
         \x20    * already been through the loader's redaction policy, which is what\n\
         \x20    * makes it something a player may be shown at all.\n\
         \x20    *\n\
         \x20    * @return the mod list, or {@code null} when no set is installed\n\
         \x20    */\n\
         \x20   public static native String ravelModSurface();\n\n\
         \x20   /**\n\
         \x20    * What opt-in profiling collected, and what it could not.\n\
         \x20    *\n\
         \x20    * <p>The third read on this framing, and the same mechanism as\n\
         \x20    * the other two: one batched read, nothing per row and nothing\n\
         \x20    * per frame.\n\
         \x20    *\n\
         \x20    * <p>The numbers cross with the sentences that qualify them, and\n\
         \x20    * every metric crosses rather than only the measured ones. A\n\
         \x20    * metric a collector cannot measure and one that measured zero\n\
         \x20    * look identical if the first is simply left out.\n\
         \x20    *\n\
         \x20    * @return the report, or {@code null} when no set is installed\n\
         \x20    */\n\
         \x20   public static native String ravelProfilingReport();\n\n\
         \x20   /**\n\
         \x20    * Admits or refuses one world's Ravel aggregate.\n\
         \x20    *\n\
         \x20    * <p>The world's open crossing. The aggregate is bytes Java never reads:\n\
         \x20    * it is handed over as it was stored and every question about what\n\
         \x20    * is inside it is answered natively, which is what keeps admission\n\
         \x20    * one authority rather than two.\n\
         \x20    *\n\
         \x20    * <p>A null array is a world holding no Ravel data. A zero-length\n\
         \x20    * one is not: it is data that is present and unreadable, and it\n\
         \x20    * refuses, because treating it as absence would let a truncated file\n\
         \x20    * silently reset the player's world.\n\
         \x20    *\n\
         \x20    * <p>The answer is empty when the world is admitted and carries the\n\
         \x20    * player-facing diagnostic when it is refused. Null is neither: it is\n\
         \x20    * what the JVM receives when the native failed, and it refuses, so a\n\
         \x20    * panic stops a world load rather than admitting one unchecked.\n\
         \x20    *\n\
         \x20    * @param aggregate the stored aggregate, or {@code null} when the\n\
         \x20    *     world holds none\n\
         \x20    * @param identity what the game calls this world, unmassaged\n\
         \x20    * @param freshWorld the game's advisory marker for a world being\n\
         \x20    *     created, which is diagnostic only and never decides admission\n\
         \x20    * @return empty when admitted, the diagnostic when refused\n\
         \x20    */\n\
         \x20   public static native String ravelOpenWorldPersistence(\n\
         \x20           byte[] aggregate, String identity, boolean freshWorld);\n\n\
         \x20   /**\n\
         \x20    * Writes the mods' data pack into a world before its packs are read.\n\
         \x20    *\n\
         \x20    * <p>A world's packs are chosen from a repository built over its\n\
         \x20    * `datapacks` directory before the open crossing above is reached,\n\
         \x20    * so a pack written there is read on the world's next open and the\n\
         \x20    * session that wrote it has none of the mods' tags, recipes or loot\n\
         \x20    * tables. This crossing is the earlier seam that closes it, and it\n\
         \x20    * carries the directory rather than a handle because the native side\n\
         \x20    * writes files.\n\
         \x20    *\n\
         \x20    * @param world the world's own directory, as the storage access\n\
         \x20    *     spells it\n\
         \x20    */\n\
         \x20   public static native void ravelWriteWorldDatapack(String world);\n\n\
         \x20   /**\n\
         \x20    * Writes the mods' data pack where the create-world screen stages\n\
         \x20    * packs.\n\
         \x20    *\n\
         \x20    * <p>Being selected and being loaded are different things. A world\n\
         \x20    * being created runs no data load of its own; it is handed the\n\
         \x20    * resources the screen loaded when it opened, so a pack that only\n\
         \x20    * reaches the world's directory is listed and unread for the whole\n\
         \x20    * first session. The screen stages packs in a directory of its own\n\
         \x20    * and copies that directory's contents into the world it makes, so\n\
         \x20    * the pack goes in there and the screen is asked to load again.\n\
         \x20    *\n\
         \x20    * @param directory the screen's staging directory, which holds pack\n\
         \x20    *     directories rather than a {@code datapacks} directory\n\
         \x20    * @return {@code 0} when the screen has no pack to load, which is\n\
         \x20    *     what no mod supplying data and a refused write both answer;\n\
         \x20    *     {@code 1} when the pack is there and holds what it already\n\
         \x20    *     held; {@code 2} when its files are not the ones that were\n\
         \x20    */\n\
         \x20   public static native int ravelWriteStagedDatapack(String directory);\n\n\
         \x20   /**\n\
         \x20    * The id the game will give the pack Ravel writes.\n\
         \x20    *\n\
         \x20    * <p>The game composes it from the name of the directory it found\n\
         \x20    * the pack in, and that name is the native side's. Asking for it\n\
         \x20    * is what keeps one launch's pack one id: the same directory is\n\
         \x20    * what a player's enabled list names and what a world being\n\
         \x20    * created has to select.\n\
         \x20    *\n\
         \x20    * @return the id, which is the same for every crossing that names\n\
         \x20    *     the pack\n\
         \x20    */\n\
         \x20   public static native String ravelPackId();\n\n\
         \x20   /**\n\
         \x20    * Offers the aggregate the saving world should now store.\n\
         \x20    *\n\
         \x20    * <p>The save crossing has the opposite failure priority to the\n\
         \x20    * open crossing: element zero tags the answer, the remainder is its\n\
         \x20    * payload, and null means the native failed and nothing is written.\n\
         \x20    * Losing one Ravel checkpoint is the safe side here, because writing\n\
         \x20    * a guess would not be.\n\
         \x20    *\n\
         \x20    * <p>{@code 0x00} unchanged, and the array is exactly one byte.\n\
         \x20    * {@code 0x01} replacement, and the remainder is the aggregate to\n\
         \x20    * store. {@code 0x02} failed and {@code 0x03} identity mismatch,\n\
         \x20    * and the remainder is a UTF-8 diagnostic; the two stay distinct\n\
         \x20    * because only the second says the lifecycle is wrong rather than a\n\
         \x20    * mod. Only {@code 0x01} writes anything.\n\
         \x20    *\n\
         \x20    * @param identity what the game calls the saving world, unmassaged\n\
         \x20    * @return the tagged answer, or {@code null} when the native failed\n\
         \x20    */\n\
         \x20   public static native byte[] ravelSaveWorldPersistence(String identity);\n\n\
         \x20   /**\n\
         \x20    * Everything Ravel's registration barrier admitted, to register.\n\
         \x20    *\n\
         \x20    * <p>The barrier decides that a mod owns an identifier and a\n\
         \x20    * registry in the running game is where that becomes a block, so\n\
         \x20    * this is the read the content adapter performs at the last point\n\
         \x20    * the built-in registries still accept entries. A hook could not\n\
         \x20    * do it: hooks only observe, and registering is an action.\n\
         \x20    *\n\
         \x20    * <p>The payload carries only the kinds a registry can take,\n\
         \x20    * because which claims are registrable is an admission question\n\
         \x20    * and admission is native. Empty is a launch with no set\n\
         \x20    * installed. Null is neither: it is what the JVM receives when\n\
         \x20    * the native failed, and nothing is registered on it.\n\
         \x20    *\n\
         \x20    * @return the admitted content, empty before a set is installed\n\
         \x20    */\n\
         \x20   public static native String ravelAdmittedContent();\n\n\
         \x20   /**\n\
         \x20    * Every action a mod asked for since this was last called.\n\
         \x20    *\n\
         \x20    * <p>The other direction of the same rule\n\
         \x20    * {@code ravelAdmittedContent} carries: a hook only observes,\n\
         \x20    * so an action a mod asks for crosses as an instruction\n\
         \x20    * and the Java adapter performs it. Every outbound action uses\n\
         \x20    * one mechanism, generic over the verb rather than written per verb.\n\
         \x20    *\n\
         \x20    * <p>Draining is what this does: an action is answered once and\n\
         \x20    * never again. Empty is a tick nobody asked for anything on.\n\
         \x20    * Null is what the JVM receives when the native failed, and\n\
         \x20    * nothing is performed on it.\n\
         \x20    *\n\
         \x20    * @return the queued actions, empty when there are none\n\
         \x20    */\n\
         \x20   public static native String ravelOutwardActions();\n\
         }"
}
